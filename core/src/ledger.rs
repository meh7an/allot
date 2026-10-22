//! The lease ledger: the queue, grants, heartbeats, and reaping.
//!
//! Pure and deterministic: every call takes the current time, nothing touches the OS or the
//! database. The caller persists the [`Transition`]s it returns.
//!
//! The queue is first in, first out with EASY backfill. When the oldest waiting lease (the
//! head) does not fit, the ledger works out when it will, from the expected ends of the
//! running leases. A later request may start before it only if it fits now and either ends
//! by then or leaves the head enough room at that time.

use rh_shared::{
    api::Verdict,
    config::Config,
    types::{Holder, JobProfile, Lease, LeaseId, LeaseKind, LeaseState, SessionKey, Snapshot},
};

use crate::admission::{self, Room, Running};

/// The config values the ledger works with, with times in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LedgerSettings {
    /// Share of every resource never handed out.
    pub headroom_pct: f32,
    /// A lease without a heartbeat for this long expires.
    pub heartbeat_ttl_ms: i64,
    /// Total CPU load at or below which the machine is quiet.
    pub quiet_cpu_pct: f32,
    /// Busiest GPU engine load at or below which the machine is quiet.
    pub quiet_gpu_pct: f32,
    /// How long the machine must stay quiet before a benchmark lease is granted.
    pub quiet_ms: i64,
}

impl LedgerSettings {
    /// Takes the admission, benchmark, and lease sections of `config`.
    pub fn from_config(config: &Config) -> Self {
        Self {
            headroom_pct: config.admission.headroom_pct,
            heartbeat_ttl_ms: secs_to_ms(config.leases.heartbeat_ttl_secs),
            quiet_cpu_pct: config.benchmark.quiet_cpu_pct,
            quiet_gpu_pct: config.benchmark.quiet_gpu_pct,
            quiet_ms: secs_to_ms(config.benchmark.quiet_secs),
        }
    }
}

fn secs_to_ms(secs: u64) -> i64 {
    i64::try_from(secs.saturating_mul(1_000)).unwrap_or(i64::MAX)
}

/// Whether the process holding a lease is still running.
pub trait ProcessLiveness {
    /// `true` when a process with the holder's PID and start time exists.
    fn is_alive(&self, holder: &Holder) -> bool;
}

/// A state change the caller must persist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    /// The lease that changed.
    pub id: LeaseId,
    /// Its new state.
    pub to: LeaseState,
    /// Why, for the audit trail.
    pub reason: String,
}

/// Requests the ledger refuses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LedgerError {
    /// No tracked lease has this ID; it may have ended already.
    #[error("lease {} is not tracked", .0.0)]
    NotFound(LeaseId),
    /// The lease belongs to another session.
    #[error("lease {} belongs to another session", .0.0)]
    NotOwner(LeaseId),
    /// Only queued leases can be enqueued.
    #[error("lease {} is not queued", .0.0)]
    NotQueued(LeaseId),
    /// A lease with this ID is already tracked.
    #[error("lease {} is already tracked", .0.0)]
    AlreadyTracked(LeaseId),
}

/// Queued and active leases, in the order they were enqueued.
#[derive(Debug, Clone)]
pub struct Ledger {
    settings: LedgerSettings,
    leases: Vec<Lease>,
    quiet_since_ms: Option<i64>,
}

#[derive(Debug, Clone)]
struct Candidate<'a> {
    profile: &'a JobProfile,
    kind: LeaseKind,
    label: String,
}

#[derive(Debug, Default)]
struct Decision {
    granted: bool,
    reasons: Vec<String>,
}

/// What the first blocked candidate of a pass protects from later ones.
#[derive(Debug)]
enum Hold {
    Free,
    /// A normal head that fits at `at_ms`, with `spare` room left beside it then.
    Shadow {
        label: String,
        at_ms: i64,
        spare: Room,
    },
    /// A benchmark head: the machine must drain, so nothing else starts.
    Exclusive(String),
}

#[derive(Debug)]
struct Pass {
    room: Room,
    running: Vec<Running>,
    active_count: usize,
    exclusive: Option<String>,
    hold: Hold,
}

impl Ledger {
    /// An empty ledger.
    pub fn new(settings: LedgerSettings) -> Self {
        Self {
            settings,
            leases: Vec::new(),
            quiet_since_ms: None,
        }
    }

    /// Queued and active leases, oldest first.
    pub fn leases(&self) -> &[Lease] {
        &self.leases
    }

    /// Adds a newly stored lease to the back of the queue. It is granted on a later
    /// [`Ledger::tick`].
    ///
    /// # Errors
    ///
    /// [`LedgerError::NotQueued`] when the lease is not `Queued`, and
    /// [`LedgerError::AlreadyTracked`] when its ID is already in the ledger.
    pub fn enqueue(&mut self, lease: Lease) -> Result<(), LedgerError> {
        if lease.state != LeaseState::Queued {
            return Err(LedgerError::NotQueued(lease.id));
        }
        if self.leases.iter().any(|tracked| tracked.id == lease.id) {
            return Err(LedgerError::AlreadyTracked(lease.id));
        }
        self.leases.push(lease);
        Ok(())
    }

    /// Moves a lease's heartbeat deadline to one TTL from `now_ms`.
    ///
    /// # Errors
    ///
    /// [`LedgerError::NotFound`] or [`LedgerError::NotOwner`].
    pub fn heartbeat(
        &mut self,
        id: LeaseId,
        session: &SessionKey,
        now_ms: i64,
    ) -> Result<&Lease, LedgerError> {
        let index = self.owned_index(id, session)?;
        let lease = &mut self.leases[index];
        lease.expires_at_ms = now_ms.saturating_add(self.settings.heartbeat_ttl_ms);
        Ok(lease)
    }

    /// Ends a queued or active lease at its holder's request and stops tracking it.
    ///
    /// # Errors
    ///
    /// [`LedgerError::NotFound`] or [`LedgerError::NotOwner`].
    pub fn release(
        &mut self,
        id: LeaseId,
        session: &SessionKey,
        now_ms: i64,
    ) -> Result<Lease, LedgerError> {
        let index = self.owned_index(id, session)?;
        let mut lease = self.leases.remove(index);
        lease.state = LeaseState::Released;
        lease.ended_at_ms = Some(now_ms);
        Ok(lease)
    }

    /// Expires leases whose holder died or whose heartbeat stopped, updates the quiet-period
    /// tracker from `snapshot`, and grants every queued lease that may start now.
    pub fn tick(
        &mut self,
        now_ms: i64,
        snapshot: &Snapshot,
        liveness: &dyn ProcessLiveness,
    ) -> Vec<Transition> {
        let mut transitions = self.reap(now_ms, liveness);
        self.observe_quiet(now_ms, snapshot);

        let candidates = self.queued_candidates();
        let decisions = self.plan(snapshot, now_ms, &candidates);
        let granted: Vec<LeaseId> = self
            .leases
            .iter()
            .filter(|lease| lease.state == LeaseState::Queued)
            .zip(decisions)
            .filter(|(_, decision)| decision.granted)
            .map(|(lease, _)| lease.id)
            .collect();

        let quiet_secs = self.settings.quiet_ms / 1_000;
        for lease in &mut self.leases {
            if granted.contains(&lease.id) {
                lease.state = LeaseState::Active;
                lease.granted_at_ms = Some(now_ms);
                let reason = if lease.kind == LeaseKind::BenchmarkExclusive {
                    format!("granted after {quiet_secs}s of quiet")
                } else {
                    "granted".to_owned()
                };
                transitions.push(Transition {
                    id: lease.id,
                    to: LeaseState::Active,
                    reason,
                });
            }
        }
        transitions
    }

    /// Whether a new request for `profile` would start now if it joined the back of the
    /// queue, with the reasons when it would not.
    ///
    /// The wait estimate assumes active leases end when expected. It is `None` when the wait
    /// depends on the queue rather than on resources, or the request can never fit.
    pub fn can_fit(
        &self,
        profile: &JobProfile,
        kind: LeaseKind,
        snapshot: &Snapshot,
        now_ms: i64,
    ) -> Verdict {
        let mut candidates = self.queued_candidates();
        let queued_ahead = candidates.len();
        candidates.push(Candidate {
            profile,
            kind,
            label: "this request".to_owned(),
        });
        let decision = self
            .plan(snapshot, now_ms, &candidates)
            .pop()
            .unwrap_or_default();
        if decision.granted {
            return Verdict {
                fits: true,
                reasons: decision.reasons,
                wait_estimate_secs: None,
            };
        }
        let mut reasons = decision.reasons;
        if queued_ahead > 0 {
            reasons.push(format!("{queued_ahead} lease(s) queued ahead"));
        }
        Verdict {
            fits: false,
            reasons,
            wait_estimate_secs: self.wait_estimate(profile, kind, snapshot, now_ms),
        }
    }

    /// Whether the tracked lease `id` would start now, with the reasons it waits. An active
    /// lease always fits. `None` when the ledger does not track `id`.
    pub fn verdict_for(&self, id: LeaseId, snapshot: &Snapshot, now_ms: i64) -> Option<Verdict> {
        let lease = self.leases.iter().find(|lease| lease.id == id)?;
        if lease.state != LeaseState::Queued {
            return Some(Verdict {
                fits: true,
                reasons: Vec::new(),
                wait_estimate_secs: None,
            });
        }
        let position = self
            .leases
            .iter()
            .filter(|lease| lease.state == LeaseState::Queued)
            .position(|lease| lease.id == id)?;
        let candidates = self.queued_candidates();
        let decision = self
            .plan(snapshot, now_ms, &candidates)
            .into_iter()
            .nth(position)?;
        if decision.granted {
            return Some(Verdict {
                fits: true,
                reasons: decision.reasons,
                wait_estimate_secs: None,
            });
        }
        let mut reasons = decision.reasons;
        if position > 0 {
            reasons.push(format!("{position} lease(s) queued ahead"));
        }
        Some(Verdict {
            fits: false,
            reasons,
            wait_estimate_secs: self.wait_estimate(&lease.profile, lease.kind, snapshot, now_ms),
        })
    }

    fn owned_index(&self, id: LeaseId, session: &SessionKey) -> Result<usize, LedgerError> {
        let index = self
            .leases
            .iter()
            .position(|lease| lease.id == id)
            .ok_or(LedgerError::NotFound(id))?;
        if &self.leases[index].holder.session != session {
            return Err(LedgerError::NotOwner(id));
        }
        Ok(index)
    }

    fn reap(&mut self, now_ms: i64, liveness: &dyn ProcessLiveness) -> Vec<Transition> {
        let ttl_secs = self.settings.heartbeat_ttl_ms / 1_000;
        let mut transitions = Vec::new();
        self.leases.retain(|lease| {
            let reason = if !liveness.is_alive(&lease.holder) {
                "holder process exited".to_owned()
            } else if now_ms >= lease.expires_at_ms {
                format!("no heartbeat for {ttl_secs}s")
            } else {
                return true;
            };
            transitions.push(Transition {
                id: lease.id,
                to: LeaseState::Expired,
                reason,
            });
            false
        });
        transitions
    }

    fn observe_quiet(&mut self, now_ms: i64, snapshot: &Snapshot) {
        let gpu_pct = snapshot
            .gpus
            .iter()
            .map(|gpu| admission::busiest_engine_pct(&gpu.engines, |load| load.avg_1s))
            .fold(0.0, f32::max);
        let quiet = snapshot.cpu.usage_pct.avg_1s <= self.settings.quiet_cpu_pct
            && gpu_pct <= self.settings.quiet_gpu_pct;
        if quiet {
            self.quiet_since_ms.get_or_insert(now_ms);
        } else {
            self.quiet_since_ms = None;
        }
    }

    /// How much longer the machine must stay quiet, or `None` when it has been quiet long
    /// enough.
    fn quiet_left_ms(&self, now_ms: i64) -> Option<i64> {
        let quiet_for = self.quiet_since_ms.map_or(0, |since| now_ms - since);
        let left = self.settings.quiet_ms - quiet_for;
        (left > 0).then_some(left)
    }

    fn active(&self) -> Vec<&Lease> {
        self.leases
            .iter()
            .filter(|lease| lease.state == LeaseState::Active)
            .collect()
    }

    fn queued_candidates(&self) -> Vec<Candidate<'_>> {
        self.leases
            .iter()
            .filter(|lease| lease.state == LeaseState::Queued)
            .map(|lease| Candidate {
                profile: &lease.profile,
                kind: lease.kind,
                label: format!("lease {}", lease.id.0),
            })
            .collect()
    }

    /// Decides every candidate in order, as if each grant happened immediately.
    fn plan(
        &self,
        snapshot: &Snapshot,
        now_ms: i64,
        candidates: &[Candidate<'_>],
    ) -> Vec<Decision> {
        let active = self.active();
        let mut pass = Pass {
            room: Room::measure(snapshot, &active, self.settings.headroom_pct),
            running: active.iter().map(|lease| Running::of(lease)).collect(),
            active_count: active.len(),
            exclusive: active
                .iter()
                .find(|lease| lease.kind == LeaseKind::BenchmarkExclusive)
                .map(|lease| format!("lease {}", lease.id.0)),
            hold: Hold::Free,
        };
        candidates
            .iter()
            .map(|candidate| self.decide(candidate, &mut pass, now_ms))
            .collect()
    }

    fn decide(&self, candidate: &Candidate<'_>, pass: &mut Pass, now_ms: i64) -> Decision {
        let blocked = |reasons: Vec<String>| Decision {
            granted: false,
            reasons,
        };
        if let Some(holder) = &pass.exclusive {
            return blocked(vec![format!("benchmark {holder} holds the machine")]);
        }
        let is_benchmark = candidate.kind == LeaseKind::BenchmarkExclusive;
        match &pass.hold {
            Hold::Exclusive(label) => {
                return blocked(vec![format!(
                    "benchmark {label} is waiting for the machine to drain"
                )]);
            }
            Hold::Shadow { label, .. } if is_benchmark => {
                return blocked(vec![format!("queued behind {label}")]);
            }
            _ => {}
        }

        let mut reasons = Vec::new();
        if is_benchmark {
            if pass.active_count > 0 {
                reasons.push(format!("{} other lease(s) still active", pass.active_count));
            }
            if let Some(left_ms) = self.quiet_left_ms(now_ms) {
                reasons.push(format!(
                    "the machine must stay quiet for {}s more",
                    admission::ms_to_secs_ceil(left_ms)
                ));
            }
        }
        reasons.extend(pass.room.shortfalls(candidate.profile));

        if reasons.is_empty() {
            if let Hold::Shadow {
                label,
                at_ms,
                spare,
            } = &mut pass.hold
            {
                let ends_ms = now_ms.saturating_add(admission::est_ms(candidate.profile));
                if ends_ms > *at_ms {
                    if !spare.shortfalls(candidate.profile).is_empty() {
                        return blocked(vec![format!(
                            "starting now would delay {label}, queued ahead"
                        )]);
                    }
                    spare.reserve(candidate.profile);
                }
            }
            pass.room.reserve(candidate.profile);
            pass.running
                .push(Running::starting(now_ms, candidate.profile));
            pass.active_count += 1;
            if is_benchmark {
                pass.exclusive = Some(candidate.label.clone());
            }
            return Decision {
                granted: true,
                reasons,
            };
        }

        if matches!(pass.hold, Hold::Free) {
            pass.hold = if is_benchmark {
                Hold::Exclusive(candidate.label.clone())
            } else {
                match admission::first_fit(pass.room, &pass.running, candidate.profile, now_ms) {
                    Some((at_ms, mut spare)) => {
                        spare.reserve(candidate.profile);
                        Hold::Shadow {
                            label: candidate.label.clone(),
                            at_ms,
                            spare,
                        }
                    }
                    None => {
                        reasons.push("more than the machine can free".to_owned());
                        Hold::Free
                    }
                }
            };
        }
        blocked(reasons)
    }

    fn wait_estimate(
        &self,
        profile: &JobProfile,
        kind: LeaseKind,
        snapshot: &Snapshot,
        now_ms: i64,
    ) -> Option<u64> {
        let active = self.active();
        let running: Vec<Running> = active.iter().map(|lease| Running::of(lease)).collect();
        if kind == LeaseKind::BenchmarkExclusive {
            let drain_ms = running
                .iter()
                .map(|reservation| reservation.end_ms - now_ms)
                .max()
                .unwrap_or(0)
                .max(0);
            let quiet_ms = if running.is_empty() {
                self.quiet_left_ms(now_ms).unwrap_or(0)
            } else {
                self.settings.quiet_ms
            };
            return Some(admission::ms_to_secs_ceil(drain_ms + quiet_ms));
        }
        let room = Room::measure(snapshot, &active, self.settings.headroom_pct);
        admission::wait_estimate_secs(room, &running, profile, now_ms).filter(|&secs| secs > 0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::test_support::{holder, profile, queued_lease, snapshot};

    const TTL_MS: i64 = 60_000;

    fn settings() -> LedgerSettings {
        LedgerSettings {
            headroom_pct: 10.0,
            heartbeat_ttl_ms: TTL_MS,
            quiet_cpu_pct: 10.0,
            quiet_gpu_pct: 10.0,
            quiet_ms: 10_000,
        }
    }

    /// Every holder is alive except the listed PIDs.
    struct Dead(HashSet<u32>);

    impl ProcessLiveness for Dead {
        fn is_alive(&self, holder: &Holder) -> bool {
            !self.0.contains(&holder.pid)
        }
    }

    fn alive() -> Dead {
        Dead(HashSet::new())
    }

    fn idle() -> Snapshot {
        snapshot(0.0, 0, None)
    }

    fn busy() -> Snapshot {
        snapshot(50.0, 0, None)
    }

    fn session(name: &str) -> SessionKey {
        holder(name).session
    }

    fn timed(cores: f32, est_secs: u64) -> JobProfile {
        JobProfile {
            est_secs,
            ..profile(cores, 1_000)
        }
    }

    fn enqueue(ledger: &mut Ledger, id: i64, cores: f32, est_secs: u64, now_ms: i64) {
        let lease = queued_lease(id, "s", timed(cores, est_secs), LeaseKind::Normal, now_ms);
        ledger.enqueue(lease).unwrap();
    }

    fn enqueue_benchmark(ledger: &mut Ledger, id: i64, now_ms: i64) {
        let lease = queued_lease(
            id,
            "bench",
            profile(1.0, 1_000),
            LeaseKind::BenchmarkExclusive,
            now_ms,
        );
        ledger.enqueue(lease).unwrap();
    }

    fn can_fit_benchmark(ledger: &Ledger, now_ms: i64) -> Verdict {
        let kind = LeaseKind::BenchmarkExclusive;
        ledger.can_fit(&profile(1.0, 1_000), kind, &idle(), now_ms)
    }

    fn granted(transitions: &[Transition]) -> Vec<i64> {
        transitions
            .iter()
            .filter(|transition| transition.to == LeaseState::Active)
            .map(|transition| transition.id.0)
            .collect()
    }

    fn states(ledger: &Ledger) -> Vec<(i64, LeaseState)> {
        ledger
            .leases()
            .iter()
            .map(|lease| (lease.id.0, lease.state))
            .collect()
    }

    fn has_reason(verdict: &Verdict, needle: &str) -> bool {
        verdict.reasons.iter().any(|reason| reason.contains(needle))
    }

    fn renew_all(ledger: &mut Ledger, now_ms: i64) {
        let ids: Vec<(LeaseId, SessionKey)> = ledger
            .leases()
            .iter()
            .map(|lease| (lease.id, lease.holder.session.clone()))
            .collect();
        for (id, session) in ids {
            ledger.heartbeat(id, &session, now_ms).unwrap();
        }
    }

    #[test]
    fn test_tick_grants_in_order_while_room_lasts() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        enqueue(&mut ledger, 2, 4.0, 300, 0);
        enqueue(&mut ledger, 3, 4.0, 300, 0);

        let transitions = ledger.tick(1_000, &idle(), &alive());
        assert_eq!(granted(&transitions), [1, 2]);
        assert_eq!(ledger.leases()[0].granted_at_ms, Some(1_000));
        assert_eq!(states(&ledger)[2], (3, LeaseState::Queued));
    }

    #[test]
    fn test_tick_background_load_blocks_until_it_drops() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        assert!(granted(&ledger.tick(1_000, &busy(), &alive())).is_empty());
        assert_eq!(granted(&ledger.tick(2_000, &idle(), &alive())), [1]);
    }

    #[test]
    fn test_release_frees_room_for_the_queued_lease() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        enqueue(&mut ledger, 2, 8.0, 300, 0);
        assert_eq!(granted(&ledger.tick(1_000, &idle(), &alive())), [1]);

        let released = ledger.release(LeaseId(1), &session("s"), 5_000).unwrap();
        assert_eq!(released.state, LeaseState::Released);
        assert_eq!(released.ended_at_ms, Some(5_000));
        assert_eq!(granted(&ledger.tick(6_000, &idle(), &alive())), [2]);
        assert_eq!(states(&ledger), [(2, LeaseState::Active)]);
    }

    #[test]
    fn test_short_request_backfills_ahead_of_a_blocked_head() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        // The head fits once lease 1 ends at 300 s; a 60 s job ends before then.
        enqueue(&mut ledger, 2, 10.0, 300, 0);
        enqueue(&mut ledger, 3, 4.0, 60, 0);
        assert_eq!(granted(&ledger.tick(1_000, &idle(), &alive())), [3]);
    }

    #[test]
    fn test_long_request_backfills_only_into_room_the_head_leaves() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        // At 300 s the head takes 10 of 14.4 cores, leaving 4.4 for jobs still running then.
        enqueue(&mut ledger, 2, 10.0, 300, 0);
        enqueue(&mut ledger, 3, 2.0, 600, 0);
        enqueue(&mut ledger, 4, 2.0, 600, 0);
        enqueue(&mut ledger, 5, 2.0, 600, 0);
        assert_eq!(granted(&ledger.tick(1_000, &idle(), &alive())), [3, 4]);

        let verdict = ledger.can_fit(&timed(2.0, 600), LeaseKind::Normal, &idle(), 1_000);
        assert!(!verdict.fits);
        assert!(has_reason(&verdict, "delay lease 2"), "{verdict:?}");
    }

    #[test]
    fn test_large_request_is_not_starved_by_a_stream_of_small_ones() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        enqueue(&mut ledger, 2, 12.0, 300, 0);

        // A long two-core job arrives every 10 s while lease 1 runs.
        for (offset, second) in (10..300).step_by(10).enumerate() {
            let now_ms = second * 1_000;
            enqueue(&mut ledger, 10 + offset as i64, 2.0, 600, now_ms);
            renew_all(&mut ledger, now_ms);
            ledger.tick(now_ms, &idle(), &alive());
        }

        ledger.release(LeaseId(1), &session("s"), 300_000).unwrap();
        renew_all(&mut ledger, 300_000);
        let transitions = ledger.tick(300_000, &idle(), &alive());
        assert_eq!(granted(&transitions).first(), Some(&2));
    }

    #[test]
    fn test_benchmark_waits_for_the_quiet_period() {
        let mut ledger = Ledger::new(settings());
        enqueue_benchmark(&mut ledger, 1, 0);
        assert!(granted(&ledger.tick(0, &busy(), &alive())).is_empty());
        assert!(granted(&ledger.tick(1_000, &idle(), &alive())).is_empty());
        assert!(granted(&ledger.tick(6_000, &idle(), &alive())).is_empty());
        assert_eq!(
            can_fit_benchmark(&ledger, 6_000).wait_estimate_secs,
            Some(5)
        );

        let transitions = ledger.tick(11_000, &idle(), &alive());
        assert_eq!(granted(&transitions), [1]);
        assert_eq!(transitions[0].reason, "granted after 10s of quiet");
    }

    #[test]
    fn test_benchmark_quiet_period_restarts_after_a_spike() {
        let mut ledger = Ledger::new(settings());
        enqueue_benchmark(&mut ledger, 1, 0);
        ledger.tick(0, &idle(), &alive());
        ledger.tick(8_000, &busy(), &alive());
        assert!(granted(&ledger.tick(12_000, &idle(), &alive())).is_empty());
        renew_all(&mut ledger, 12_000);
        assert_eq!(granted(&ledger.tick(22_000, &idle(), &alive())), [1]);
    }

    #[test]
    fn test_benchmark_drains_the_machine_and_then_runs_alone() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 4.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        enqueue_benchmark(&mut ledger, 2, 0);
        enqueue(&mut ledger, 3, 1.0, 10, 0);

        assert!(granted(&ledger.tick(20_000, &idle(), &alive())).is_empty());
        let waiting = ledger.can_fit(&profile(1.0, 1_000), LeaseKind::Normal, &idle(), 20_000);
        assert!(
            has_reason(&waiting, "waiting for the machine to drain"),
            "{waiting:?}"
        );

        ledger.release(LeaseId(1), &session("s"), 30_000).unwrap();
        renew_all(&mut ledger, 30_000);
        assert_eq!(granted(&ledger.tick(30_000, &idle(), &alive())), [2]);

        let held = ledger.can_fit(&profile(1.0, 1_000), LeaseKind::Normal, &idle(), 31_000);
        assert!(!held.fits);
        assert!(
            has_reason(&held, "benchmark lease 2 holds the machine"),
            "{held:?}"
        );
        assert_eq!(states(&ledger)[1], (3, LeaseState::Queued));
    }

    #[test]
    fn test_benchmark_wait_estimate_is_drain_plus_quiet() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 4.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        let verdict = can_fit_benchmark(&ledger, 100_000);
        assert!(!verdict.fits);
        assert_eq!(verdict.wait_estimate_secs, Some(210));
    }

    #[test]
    fn test_can_fit_reports_shortfall_queue_and_wait() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        enqueue(&mut ledger, 2, 12.0, 300, 0);

        let verdict = ledger.can_fit(&profile(8.0, 1_000), LeaseKind::Normal, &idle(), 100_000);
        assert!(!verdict.fits);
        assert!(has_reason(&verdict, "cores:"), "{verdict:?}");
        assert!(
            has_reason(&verdict, "1 lease(s) queued ahead"),
            "{verdict:?}"
        );
        assert_eq!(verdict.wait_estimate_secs, Some(200));

        let small = ledger.can_fit(&timed(1.0, 30), LeaseKind::Normal, &idle(), 100_000);
        assert_eq!((small.fits, small.wait_estimate_secs), (true, None));
    }

    #[test]
    fn test_can_fit_never_fitting_request_has_no_estimate() {
        let ledger = Ledger::new(settings());
        let verdict = ledger.can_fit(&profile(64.0, 1_000), LeaseKind::Normal, &idle(), 0);
        assert!(!verdict.fits);
        assert!(
            has_reason(&verdict, "more than the machine can free"),
            "{verdict:?}"
        );
        assert_eq!(verdict.wait_estimate_secs, None);
    }

    #[test]
    fn test_missing_heartbeat_expires_queued_and_active_leases() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        enqueue(&mut ledger, 2, 12.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        assert!(ledger.tick(TTL_MS - 1, &idle(), &alive()).is_empty());

        let transitions = ledger.tick(TTL_MS, &idle(), &alive());
        let expired = [1, 2].map(|id| Transition {
            id: LeaseId(id),
            to: LeaseState::Expired,
            reason: "no heartbeat for 60s".into(),
        });
        assert_eq!(transitions, expired);
        assert!(ledger.leases().is_empty());
    }

    #[test]
    fn test_heartbeat_extends_the_deadline() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        let lease = ledger.heartbeat(LeaseId(1), &session("s"), 30_000).unwrap();
        assert_eq!(lease.expires_at_ms, 30_000 + TTL_MS);
        assert_eq!(granted(&ledger.tick(TTL_MS, &idle(), &alive())), [1]);
        let transitions = ledger.tick(30_000 + TTL_MS, &idle(), &alive());
        assert_eq!(transitions[0].to, LeaseState::Expired);
    }

    #[test]
    fn test_dead_holder_is_reaped_and_frees_room() {
        let mut ledger = Ledger::new(settings());
        let doomed = queued_lease(1, "doomed", profile(8.0, 1_000), LeaseKind::Normal, 0);
        let waiting = queued_lease(2, "alive", profile(8.0, 1_000), LeaseKind::Normal, 0);
        ledger.enqueue(doomed).unwrap();
        ledger.enqueue(waiting).unwrap();
        assert_eq!(granted(&ledger.tick(0, &idle(), &alive())), [1]);

        let dead = Dead(HashSet::from([holder("doomed").pid]));
        let transitions = ledger.tick(1_000, &idle(), &dead);
        let reaped = Transition {
            id: LeaseId(1),
            to: LeaseState::Expired,
            reason: "holder process exited".into(),
        };
        assert_eq!(transitions[0], reaped);
        assert_eq!(granted(&transitions), [2]);
    }

    #[test]
    fn test_ledger_refuses_bad_requests() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 1.0, 300, 0);

        let duplicate = queued_lease(1, "s", profile(1.0, 1), LeaseKind::Normal, 0);
        let error = ledger.enqueue(duplicate);
        assert_eq!(error, Err(LedgerError::AlreadyTracked(LeaseId(1))));
        let mut active = queued_lease(2, "s", profile(1.0, 1), LeaseKind::Normal, 0);
        active.state = LeaseState::Active;
        assert_eq!(
            ledger.enqueue(active),
            Err(LedgerError::NotQueued(LeaseId(2)))
        );

        let error = ledger.release(LeaseId(1), &session("other"), 0);
        assert_eq!(error, Err(LedgerError::NotOwner(LeaseId(1))));
        let error = ledger
            .heartbeat(LeaseId(9), &session("s"), 0)
            .map(|lease| lease.id);
        assert_eq!(error, Err(LedgerError::NotFound(LeaseId(9))));
        assert_eq!(
            LedgerError::NotOwner(LeaseId(1)).to_string(),
            "lease 1 belongs to another session"
        );
    }

    #[test]
    fn test_verdict_for_explains_each_tracked_lease() {
        let mut ledger = Ledger::new(settings());
        enqueue(&mut ledger, 1, 8.0, 300, 0);
        ledger.tick(0, &idle(), &alive());
        enqueue(&mut ledger, 2, 12.0, 300, 0);
        enqueue(&mut ledger, 3, 12.0, 300, 0);

        let running = ledger.verdict_for(LeaseId(1), &idle(), 1_000).unwrap();
        assert!(running.fits && running.reasons.is_empty());
        let head = ledger.verdict_for(LeaseId(2), &idle(), 1_000).unwrap();
        assert!(!head.fits);
        assert!(has_reason(&head, "cores:"), "{head:?}");
        assert_eq!(head.wait_estimate_secs, Some(299));
        let second = ledger.verdict_for(LeaseId(3), &idle(), 1_000).unwrap();
        assert!(has_reason(&second, "1 lease(s) queued ahead"), "{second:?}");
        assert_eq!(ledger.verdict_for(LeaseId(9), &idle(), 1_000), None);
    }

    #[test]
    fn test_settings_from_config_converts_seconds() {
        let settings = LedgerSettings::from_config(&Config::default());
        assert_eq!(settings.heartbeat_ttl_ms, 60_000);
        assert_eq!(settings.quiet_ms, 10_000);
        assert_eq!(settings.headroom_pct, 15.0);
    }
}
