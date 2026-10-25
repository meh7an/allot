//! Ending leases their holder forgot to release: no command runs under them any more and the
//! holder's session has been idle for a while.
//!
//! A forgotten lease never expires on its own, because the MCP server keeps sending its
//! heartbeat while the session lives. Its reservation blocks every other session until the
//! session ends, so the daemon ends it instead.
//!
//! Quick leases get the same treatment once they run far past what made them quick: they may
//! start beside a waiting benchmark only because they end soon.

use std::time::Duration;

use allot_shared::types::{Lease, LeaseId, LeaseKind, LeaseState, ResourceUsage, Snapshot};

use crate::{
    binding::{self, BindOutcome},
    process::ProcessTable,
};

/// A session using fewer busy cores than this is idle. A `claude.exe` waiting for the model
/// uses a few hundredths of a core.
pub const IDLE_CORES: f32 = 1.0;

/// A session using less GPU than this, as a percentage of one adapter, is idle.
pub const IDLE_GPU_PCT: f32 = 5.0;

/// When the holder of each active lease went idle.
#[derive(Debug, Clone, Default)]
pub struct IdleWatch {
    since: Vec<(LeaseId, i64)>,
}

impl IdleWatch {
    /// An empty watch.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one sample and returns the active leases idle for at least `after`, each with
    /// the time it went idle. A zero `after` turns the watch off.
    ///
    /// A lease is idle while no command bound to it runs or may still start, and its
    /// holder's session stays below [`IDLE_CORES`] and [`IDLE_GPU_PCT`]. A lease no command
    /// was bound to only needs the idle session. Any busy sample starts the wait over.
    pub fn observe(
        &mut self,
        leases: &[Lease],
        snapshot: &Snapshot,
        table: &ProcessTable,
        after: Duration,
    ) -> Vec<(LeaseId, i64)> {
        let previous = std::mem::take(&mut self.since);
        if after.is_zero() {
            return Vec::new();
        }
        let now_ms = snapshot.taken_at_ms;
        let after_ms = i64::try_from(after.as_millis()).unwrap_or(i64::MAX);
        let mut due = Vec::new();
        for lease in leases
            .iter()
            .filter(|lease| lease.state == LeaseState::Active)
        {
            if command_may_run(lease, table, now_ms) || !is_idle(&session_usage(lease, snapshot)) {
                continue;
            }
            let since = previous
                .iter()
                .find(|(id, _)| *id == lease.id)
                .map_or(now_ms, |&(_, since)| since);
            // Kept while due, so a lease the caller failed to end comes back on the next sample.
            self.since.push((lease.id, since));
            if now_ms.saturating_sub(since) >= after_ms {
                due.push((lease.id, since));
            }
        }
        due
    }
}

/// Whether the command bound to `lease` runs, or may still start, at `now_ms`.
fn command_may_run(lease: &Lease, table: &ProcessTable, now_ms: i64) -> bool {
    lease.binding.as_ref().is_some_and(|current| {
        binding::resolve(current, &lease.holder, table, now_ms) != BindOutcome::Ended
    })
}

/// What the holder's session uses in `snapshot`; nothing when the sample does not show it.
fn session_usage(lease: &Lease, snapshot: &Snapshot) -> ResourceUsage {
    let mut usage = ResourceUsage::default();
    for session in &snapshot.sessions {
        if session.holder.session == lease.holder.session {
            usage += session.usage;
        }
    }
    usage
}

fn is_idle(usage: &ResourceUsage) -> bool {
    usage.cores < IDLE_CORES && usage.gpu_pct < IDLE_GPU_PCT
}

/// How long a quick lease may stay active before the daemon ends it. The run it records then
/// lasts this long, so history no longer estimates the command as quick.
pub const QUICK_OVERRUN_MS: i64 = 60_000;

/// The active quick leases granted at least [`QUICK_OVERRUN_MS`] before `now_ms`. Low-impact
/// leases are never cut short: they hold nothing back.
pub fn quick_overruns(leases: &[Lease], now_ms: i64) -> Vec<LeaseId> {
    leases
        .iter()
        .filter(|lease| {
            lease.state == LeaseState::Active
                && lease.kind == LeaseKind::Normal
                && lease.profile.is_quick()
                && !lease.profile.is_low_impact()
                && lease
                    .granted_at_ms
                    .is_some_and(|granted| now_ms.saturating_sub(granted) >= QUICK_OVERRUN_MS)
        })
        .map(|lease| lease.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use allot_shared::types::{BoundProcess, JobProfile, LeaseBinding, SessionUsage};

    use super::*;
    use crate::test_support::{active_lease, holder, profile, raw_process, session_usage};

    const SESSION: &str = "a";
    const AFTER: Duration = Duration::from_secs(180);
    /// The holder runs as PID 1097 (see `holder`); its command's shell as PID 2000.
    const SHELL: u32 = 2_000;

    fn lease(binding: Option<LeaseBinding>) -> Lease {
        Lease {
            binding,
            ..active_lease(1, SESSION, profile(8.0, 4_096))
        }
    }

    /// A command reported at 10 s whose shell started at 10 s.
    fn bound() -> Option<LeaseBinding> {
        Some(LeaseBinding {
            tool_use_id: "toolu_01".into(),
            bound_at_ms: 10_000,
            root: Some(BoundProcess {
                pid: SHELL,
                start_time: 10,
            }),
        })
    }

    /// The holder, plus the command's shell while it runs.
    fn table(shell_running: bool) -> ProcessTable {
        let session = holder(SESSION);
        let mut processes = vec![raw_process(session.pid, None, "claude.exe", 1)];
        if shell_running {
            processes.push(raw_process(SHELL, Some(session.pid), "bash.exe", 10));
        }
        ProcessTable::new(&processes)
    }

    fn sample(at_ms: i64, cores: f32) -> Snapshot {
        Snapshot {
            taken_at_ms: at_ms,
            sessions: vec![session_usage(SESSION, cores)],
            ..crate::test_support::snapshot(10.0, 8_192, None)
        }
    }

    /// Feeds one sample a second from `from_ms` up to `to_ms` and returns the last answer.
    fn run(
        watch: &mut IdleWatch,
        leases: &[Lease],
        table: &ProcessTable,
        from_ms: i64,
        to_ms: i64,
        cores: f32,
    ) -> Vec<(LeaseId, i64)> {
        let mut due = Vec::new();
        for at_ms in (from_ms..=to_ms).step_by(1_000) {
            due = watch.observe(leases, &sample(at_ms, cores), table, AFTER);
        }
        due
    }

    #[test]
    fn test_observe_ends_a_lease_once_its_command_ended_and_the_session_idled() {
        let mut watch = IdleWatch::new();
        let leases = [lease(bound())];
        assert!(run(&mut watch, &leases, &table(true), 10_000, 60_000, 0.02).is_empty());
        assert!(run(&mut watch, &leases, &table(false), 61_000, 240_000, 0.02).is_empty());
        let due = watch.observe(&leases, &sample(241_000, 0.02), &table(false), AFTER);
        assert_eq!(due, [(LeaseId(1), 61_000)]);
    }

    #[test]
    fn test_observe_keeps_a_lease_while_its_command_runs_even_idle() {
        let mut watch = IdleWatch::new();
        let leases = [lease(bound())];
        assert!(run(&mut watch, &leases, &table(true), 10_000, 400_000, 0.0).is_empty());
    }

    #[test]
    fn test_observe_keeps_a_lease_while_its_command_may_still_start() {
        let mut watch = IdleWatch::new();
        let mut pending = bound();
        if let Some(binding) = &mut pending {
            binding.root = None;
            binding.bound_at_ms = 395_000;
        }
        let leases = [lease(pending)];
        assert!(run(&mut watch, &leases, &table(false), 200_000, 400_000, 0.0).is_empty());
    }

    #[test]
    fn test_observe_starts_the_wait_over_after_a_busy_sample() {
        let mut watch = IdleWatch::new();
        let leases = [lease(bound())];
        let ended = table(false);
        run(&mut watch, &leases, &ended, 20_000, 190_000, 0.1);
        watch.observe(&leases, &sample(191_000, 6.0), &ended, AFTER);
        assert!(run(&mut watch, &leases, &ended, 192_000, 371_000, 0.1).is_empty());
        let due = watch.observe(&leases, &sample(372_000, 0.1), &ended, AFTER);
        assert_eq!(due, [(LeaseId(1), 192_000)]);
    }

    #[test]
    fn test_observe_counts_gpu_load_as_busy() {
        let mut watch = IdleWatch::new();
        let leases = [lease(None)];
        let table = table(false);
        let busy_gpu = Snapshot {
            sessions: vec![SessionUsage {
                usage: ResourceUsage {
                    gpu_pct: 40.0,
                    ..ResourceUsage::default()
                },
                ..session_usage(SESSION, 0.1)
            }],
            ..sample(0, 0.0)
        };
        for at_ms in (0..=400_000).step_by(1_000) {
            let sample = Snapshot {
                taken_at_ms: at_ms,
                ..busy_gpu.clone()
            };
            assert!(watch.observe(&leases, &sample, &table, AFTER).is_empty());
        }
    }

    #[test]
    fn test_observe_ends_an_unbound_lease_on_idle_alone() {
        let mut watch = IdleWatch::new();
        let leases = [lease(None)];
        let due = run(&mut watch, &leases, &table(false), 0, 180_000, 0.3);
        assert_eq!(due, [(LeaseId(1), 0)]);
    }

    #[test]
    fn test_observe_reports_a_due_lease_again_until_it_ends() {
        let mut watch = IdleWatch::new();
        let leases = [lease(None)];
        run(&mut watch, &leases, &table(false), 0, 180_000, 0.0);
        let again = watch.observe(&leases, &sample(181_000, 0.0), &table(false), AFTER);
        assert_eq!(again, [(LeaseId(1), 0)]);
        let ended = Lease {
            state: LeaseState::Expired,
            ..lease(None)
        };
        let gone = watch.observe(&[ended], &sample(182_000, 0.0), &table(false), AFTER);
        assert!(gone.is_empty());
        assert!(watch.since.is_empty());
    }

    #[test]
    fn test_observe_sums_the_holder_session_only() {
        let mut watch = IdleWatch::new();
        let leases = [lease(None)];
        for at_ms in (0..=400_000).step_by(1_000) {
            let sample = Snapshot {
                sessions: vec![
                    session_usage("other", 12.0),
                    session_usage(SESSION, 0.6),
                    session_usage(SESSION, 0.6),
                ],
                ..sample(at_ms, 0.0)
            };
            assert!(
                watch
                    .observe(&leases, &sample, &table(false), AFTER)
                    .is_empty()
            );
        }
    }

    #[test]
    fn test_observe_off_when_after_is_zero() {
        let mut watch = IdleWatch::new();
        let leases = [lease(None)];
        for at_ms in (0..=400_000).step_by(10_000) {
            let due = watch.observe(&leases, &sample(at_ms, 0.0), &table(false), Duration::ZERO);
            assert!(due.is_empty());
        }
    }

    #[test]
    fn test_observe_ignores_queued_leases() {
        let mut watch = IdleWatch::new();
        let leases = [Lease {
            state: LeaseState::Queued,
            ..lease(None)
        }];
        assert!(run(&mut watch, &leases, &table(false), 0, 400_000, 0.0).is_empty());
    }

    #[test]
    fn test_quick_overruns_ends_quick_leases_after_a_minute_only() {
        let with = |id, cores, ram_mb, est_secs, kind| Lease {
            kind,
            granted_at_ms: Some(1_000),
            ..active_lease(
                id,
                SESSION,
                JobProfile {
                    est_secs,
                    ..profile(cores, ram_mb)
                },
            )
        };
        let leases = [
            with(1, 4.0, 2_048, 10, LeaseKind::Normal),
            with(2, 4.0, 2_048, 300, LeaseKind::Normal),
            with(3, 0.25, 200, 10, LeaseKind::Normal),
            with(4, 1.0, 1_000, 10, LeaseKind::BenchmarkExclusive),
        ];
        assert!(quick_overruns(&leases, 60_999).is_empty());
        assert_eq!(quick_overruns(&leases, 61_000), [LeaseId(1)]);
    }
}
