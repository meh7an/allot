use serde::{Deserialize, Serialize};

use crate::{
    config::FieldError,
    config_write::{Change, Revision},
    explain::Shell,
    settings::{SettingValue, SettingsSpec},
    types::{
        EndCause, Holder, JobClass, JobProfile, JobRun, Lease, LeaseKind, LeaseState, NoiseReport,
        SessionKey, SessionUsage,
    },
    ui::Notice,
};

/// Response of `GET /health`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Daemon version, from its `Cargo.toml`.
    pub version: String,
    /// Daemon process ID.
    pub pid: u32,
    /// Seconds since the daemon started.
    pub uptime_secs: u64,
    /// Why the last change to `config.toml` was rejected; the daemon keeps the config it had
    /// until the file is fixed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_error: Option<String>,
}

/// How a request states the resources a job needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProfileSpec {
    /// A named class, resolved through the daemon's config.
    Class(JobClass),
    /// A named preset from the daemon's config, for example `cpu_small`. The lease records it
    /// as a custom profile with no class.
    Preset(String),
    /// Explicit values.
    Custom(JobProfile),
}

/// Body of `POST /can-fit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanFitRequest {
    /// Resources the job needs.
    pub profile: ProfileSpec,
    /// How the job would share the machine.
    pub kind: LeaseKind,
    /// The fewest cores the job can start with. Without it the class or preset's minimum
    /// applies, and a custom profile keeps all its cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cores: Option<f32>,
    /// The command, so the daemon can raise the profile from similar past runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Working directory of the command; part of what makes runs similar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// The reason a verdict gives when ending every lease would still leave too little room, so
/// only something outside Allot can make the request fit, if anything can.
pub const REASON_BEYOND_LEASES: &str = "more than the machine can free";

/// The admission engine's answer. Response of `POST /can-fit`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    /// Whether the job could start now.
    pub fits: bool,
    /// Human-readable explanations, one per limiting or notable factor.
    pub reasons: Vec<String>,
    /// Expected wait before the job fits, when the daemon can estimate it.
    pub wait_estimate_secs: Option<u64>,
    /// Where a benchmark at the head of the queue stands; `None` for other requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BenchmarkProgress>,
}

/// Where a waiting benchmark lease stands, so its holder can report progress.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
#[non_exhaustive]
pub enum BenchmarkProgress {
    /// Other leases are still active; nothing new starts until they end.
    Draining {
        /// Active leases left.
        active_leases: u32,
    },
    /// The machine has drained and its background load is being averaged.
    Measuring {
        /// Seconds averaged so far.
        measured_secs: u64,
        /// Seconds the average must cover.
        needed_secs: u64,
        /// Average total CPU load so far, in percent.
        cpu_avg_pct: f32,
        /// Average load of the busiest GPU engine so far, in percent.
        gpu_avg_pct: f32,
    },
    /// The average is above the quiet limits. The lease is granted anyway, marked as not
    /// quiet, once `grant_anyway_in_secs` have passed.
    Noisy {
        /// Average total CPU load over the quiet window, in percent.
        cpu_avg_pct: f32,
        /// Average load of the busiest GPU engine over the quiet window, in percent.
        gpu_avg_pct: f32,
        /// CPU limit of a quiet machine.
        cpu_limit_pct: f32,
        /// GPU limit of a quiet machine.
        gpu_limit_pct: f32,
        /// Seconds until the lease is granted without quiet.
        grant_anyway_in_secs: u64,
    },
}

/// Body of `POST /leases`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcquireRequest {
    /// Session asking for the lease.
    pub holder: Holder,
    /// Resources to reserve.
    pub profile: ProfileSpec,
    /// The fewest cores the job can start with. Without it the class or preset's minimum
    /// applies, and a custom profile keeps all its cores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_cores: Option<f32>,
    /// How the lease shares the machine.
    pub kind: LeaseKind,
    /// What the lease is for, usually the command that will run.
    pub label: String,
    /// Working directory of the command; part of what makes runs similar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// How long the daemon may hold the request open waiting for the lease to become active.
    pub wait_timeout_secs: u64,
}

/// Response of `POST /leases`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcquireResponse {
    /// The lease, `Active` if it was granted within the wait timeout and `Queued` otherwise.
    pub lease: Lease,
    /// Why the lease was granted or is still waiting.
    pub verdict: Verdict,
}

/// Body of `POST /leases/{id}/wait`: keep waiting for a queued lease without losing its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitRequest {
    /// How long the daemon may hold the request open waiting for the lease to become active.
    pub wait_timeout_secs: u64,
}

/// Query string of `POST /leases/{id}/heartbeat`, `POST /leases/{id}/wait`,
/// `POST /leases/{id}/bind`, `DELETE /leases/{id}`, `GET /notices`, and
/// `POST /notices/{id}/delivered`.
///
/// The daemon refuses a lease or notice call when the lease belongs to another session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseOwner {
    /// Session that claims to own the lease.
    pub session: SessionKey,
}

/// Response of `POST /leases/{id}/heartbeat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    /// Current state, so a holder of a queued lease learns when it becomes active.
    pub state: LeaseState,
    /// New expiry, in milliseconds since the Unix epoch.
    pub expires_at_ms: i64,
    /// Why the lease ended, once it has; for example [`EndCause::Killed`] when the user
    /// stopped its job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_cause: Option<EndCause>,
}

/// Response of `DELETE /leases/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseResponse {
    /// The lease in its final state.
    pub lease: Lease,
    /// Background load while a benchmark lease was active. Only the release that ends the
    /// lease carries it, and only when at least one sample was taken during the lease.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise: Option<NoiseReport>,
    /// What the run used. Only the release that ends an active lease carries it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunReport>,
    /// What the next run of the same request would reserve, now that this run counts. Only
    /// with `run`, and only while allotd still knows the request, which a lease restored
    /// across a daemon restart it does not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<NextReservation>,
}

/// What one finished run used.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RunReport {
    /// Cores and GPU load while its session was busy, peak RAM and VRAM, and run time in
    /// `est_secs`: the values [`RunStats`] takes of each run.
    pub used: JobProfile,
    /// Whether estimates count the run; one that overlapped another lease of its session or
    /// that the user killed does not.
    pub counted: bool,
}

/// What a request would reserve if it were made again now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NextReservation {
    /// The profile acquire would reserve, before any shrink to fit.
    pub profile: JobProfile,
    /// What history changed, worded as acquire words it; empty when nothing changed.
    pub reasons: Vec<String>,
}

/// Response of `POST /leases/{id}/kill`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KillResponse {
    /// The lease, ended with cause `killed`.
    pub lease: Lease,
    /// Processes terminated, root first.
    pub terminated: Vec<KilledProcess>,
    /// Processes that could not be terminated, with what the OS said.
    pub failed: Vec<UnkilledProcess>,
    /// Processes in the tree left running on purpose, each with everything below it.
    pub spared: Vec<UnkilledProcess>,
    /// Why no process was terminated: the lease had no bound process tree, or its command had
    /// already ended. The lease ended anyway. `None` when the bound tree was killed.
    pub untouched_reason: Option<String>,
}

/// A process a kill terminated.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KilledProcess {
    /// Process ID.
    pub pid: u32,
    /// Executable name, for example `cargo.exe`.
    pub name: String,
}

/// A process in a killed tree that is still running.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnkilledProcess {
    /// Process ID.
    pub pid: u32,
    /// Executable name, for example `claude.exe`.
    pub name: String,
    /// Why it is still running.
    pub reason: String,
}

/// Response of `POST /leases/{id}/remove`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoveResponse {
    /// The lease, ended with cause `removed` before it started.
    pub lease: Lease,
}

/// Body of `POST /leases/{id}/bind`: the PreToolUse hook let a command run under the lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindRequest {
    /// Claude Code's ID for the tool call that runs the command.
    pub tool_use_id: String,
    /// The command as the hook saw it, for the log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// Response of `GET /notices`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticesResponse {
    /// Kills and removals the session has not been told about yet, oldest first.
    pub notices: Vec<Notice>,
}

/// Response of `POST /notices/{id}/delivered`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveredResponse {
    /// `true` when this call recorded the delivery; `false` when an earlier one already had,
    /// so the session must not be told again.
    pub delivered: bool,
}

/// Query string of `GET /leases`. Each filter is optional; an empty query lists every lease.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LeaseQuery {
    /// Only leases held by this session.
    pub session: Option<SessionKey>,
    /// Only leases in this state.
    pub state: Option<LeaseState>,
}

/// Response of `GET /leases`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeasesResponse {
    /// Matching leases, oldest first.
    pub leases: Vec<Lease>,
}

/// Response of `GET /sessions`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionsResponse {
    /// Every registered session with its current usage.
    pub sessions: Vec<SessionUsage>,
}

/// Body of `POST /sessions`. Registering the same session again is harmless; the daemon
/// forgets a session once its holder process is gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterSessionRequest {
    /// The session and its `claude.exe`.
    pub holder: Holder,
}

/// Response of `GET /history`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryResponse {
    /// One entry per fingerprint, most recently run first.
    pub jobs: Vec<JobSummary>,
}

/// What the history knows about one fingerprint.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobSummary {
    /// Normalized command plus working directory.
    pub fingerprint: String,
    /// Label of the most recent run.
    pub command: String,
    /// Working directory of the most recent run.
    pub cwd: String,
    /// Stored runs.
    pub runs: u32,
    /// Runs the estimate is based on: the most recent ones without an overlapping lease.
    pub estimate_runs: u32,
    /// End of the most recent run, in milliseconds since the Unix epoch.
    pub last_ended_at_ms: i64,
    /// What the runs needed: p90 of peak memory, of CPU and GPU load while busy, and of run
    /// time. `None` when every run overlapped another lease. The same as `stats.p90`.
    pub learned: Option<JobProfile>,
    /// Median, p90 and max over the runs the estimate is based on; `None` when every run
    /// overlapped another lease, and from older daemons.
    #[serde(default)]
    pub stats: Option<RunStats>,
}

/// Median, p90 and max of what a job's runs used: the newest runs that neither overlapped
/// another lease of their session nor were killed, at most as many as an estimate reads.
/// Cores and GPU load are each run's average while its session was busy, RAM and VRAM its
/// peak, and `est_secs` its run time.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RunStats {
    /// Runs the statistics come from.
    pub runs: u32,
    /// Nearest-rank median of each value.
    pub median: JobProfile,
    /// Nearest-rank 90th percentile of each value, which estimates raise and lower to.
    pub p90: JobProfile,
    /// Highest of each value.
    pub max: JobProfile,
}

/// Query string of `GET /history/runs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRunsQuery {
    /// Fingerprint whose runs to list.
    pub fingerprint: String,
    /// Most runs to return; the daemon's default when absent.
    pub limit: Option<u32>,
}

/// Response of `GET /history/runs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryRunsResponse {
    /// Runs with the fingerprint, newest first.
    pub runs: Vec<JobRun>,
    /// Statistics of the fingerprint over the runs an estimate reads, which may reach past
    /// `runs`; `None` when no run counts, and from older daemons.
    #[serde(default)]
    pub stats: Option<RunStats>,
}

/// Response of `GET /settings` and of a `PATCH /settings` that was written.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsState {
    /// Every setting allotd owns, with its page, control, range, default, label and help.
    pub spec: SettingsSpec,
    /// The value of every setting on the spec's pages: the file's, or while allotd rejects the
    /// file, the last good values, which stay in effect.
    pub values: Vec<SettingValue>,
    /// The revision of `config.toml`, to send with the next `PATCH /settings`.
    pub revision: Revision,
    /// Why allotd rejects `config.toml`, if it does. Settings cannot be changed until the
    /// file is fixed by hand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_error: Option<String>,
    /// The port allotd listens on, which differs from `daemon.port` until a restart.
    pub listening_port: u32,
    /// Keys whose saved value takes effect only once allotd restarts, such as `daemon.port`.
    pub pending_restart: Vec<String>,
}

/// Body of `PATCH /settings`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsPatch {
    /// The revision the changes were made against, from the last `GET /settings`.
    pub revision: Revision,
    /// The changes, written together or not at all. A reset is a change too.
    pub changes: Vec<Change>,
}

/// Body of `POST /settings/preview`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PreviewRequest {
    /// Unsaved changes, applied on top of the file; none previews the file as it is.
    #[serde(default)]
    pub changes: Vec<Change>,
}

/// Body of `POST /classify`.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifyRequest {
    /// The command line to test.
    pub line: String,
    /// The shell the line is written for.
    #[serde(default)]
    pub shell: Shell,
    /// Unsaved changes, such as edited rules, applied on top of the file.
    #[serde(default)]
    pub changes: Vec<Change>,
}

/// Body of a refused settings request. Nothing was written.
#[cfg_attr(any(feature = "schema", test), derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsRefusal {
    /// What went wrong, as one sentence.
    pub message: String,
    /// Every rejected value with its key, when values were the cause; empty otherwise.
    pub problems: Vec<FieldError>,
    /// The file's revision now, when it changed since the request's revision was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        assert_json_round_trip, sample_holder, sample_lease, sample_session_usage,
    };

    fn sample_verdict() -> Verdict {
        Verdict {
            fits: false,
            reasons: vec!["RAM: 6144 MB requested, 4096 MB free after headroom".into()],
            wait_estimate_secs: Some(90),
            benchmark: None,
        }
    }

    fn custom_profile() -> ProfileSpec {
        ProfileSpec::Custom(JobProfile {
            cores: 3.5,
            ram_mb: 6144,
            gpu_pct: 40.0,
            vram_mb: 2048,
            est_secs: 120,
        })
    }

    #[test]
    fn test_health_response_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&HealthResponse {
            version: "0.1.0".into(),
            pid: 4242,
            uptime_secs: 3600,
            config_error: Some("config.toml: invalid configuration: x".into()),
        });
    }

    #[test]
    fn test_can_fit_request_json_round_trip_preserves_both_profile_forms() {
        assert_json_round_trip(&CanFitRequest {
            profile: ProfileSpec::Class(JobClass::GpuHeavy),
            kind: LeaseKind::Normal,
            min_cores: None,
            label: None,
            cwd: None,
        });
        assert_json_round_trip(&CanFitRequest {
            profile: custom_profile(),
            kind: LeaseKind::BenchmarkExclusive,
            min_cores: Some(2.0),
            label: Some("cargo bench".into()),
            cwd: Some(r"E:\work\app".into()),
        });
    }

    #[test]
    fn test_can_fit_request_without_label_and_cwd_deserializes_as_none() {
        let request: CanFitRequest =
            serde_json::from_str(r#"{"profile":{"class":"light"},"kind":"normal"}"#).unwrap();
        assert_eq!((request.label, request.cwd), (None, None));
    }

    #[test]
    fn test_profile_spec_json_is_externally_tagged() {
        let class = serde_json::to_value(ProfileSpec::Class(JobClass::CpuHeavy)).unwrap();
        assert_eq!(class, serde_json::json!({ "class": "cpu_heavy" }));
        let custom = serde_json::to_value(custom_profile()).unwrap();
        assert_eq!(custom["custom"]["ram_mb"], 6144);
    }

    #[test]
    fn test_verdict_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&sample_verdict());
        assert_json_round_trip(&Verdict {
            fits: true,
            reasons: vec![],
            wait_estimate_secs: None,
            benchmark: None,
        });
    }

    #[test]
    fn test_verdict_benchmark_progress_round_trips_every_phase() {
        let phases = [
            BenchmarkProgress::Draining { active_leases: 2 },
            BenchmarkProgress::Measuring {
                measured_secs: 4,
                needed_secs: 10,
                cpu_avg_pct: 6.5,
                gpu_avg_pct: 0.0,
            },
            BenchmarkProgress::Noisy {
                cpu_avg_pct: 12.5,
                gpu_avg_pct: 1.0,
                cpu_limit_pct: 10.0,
                gpu_limit_pct: 10.0,
                grant_anyway_in_secs: 35,
            },
        ];
        for phase in phases {
            assert_json_round_trip(&Verdict {
                benchmark: Some(phase),
                ..sample_verdict()
            });
        }
        let value = serde_json::to_value(phases[0]).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "phase": "draining", "active_leases": 2 })
        );
    }

    #[test]
    fn test_verdict_without_benchmark_field_deserializes_as_none() {
        let verdict: Verdict =
            serde_json::from_str(r#"{"fits":true,"reasons":[],"wait_estimate_secs":null}"#)
                .unwrap();
        assert_eq!(verdict.benchmark, None);
    }

    #[test]
    fn test_acquire_request_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&AcquireRequest {
            holder: sample_holder(),
            profile: custom_profile(),
            min_cores: Some(1.5),
            kind: LeaseKind::Normal,
            label: "cargo build --release".into(),
            cwd: Some(r"E:\work\app".into()),
            wait_timeout_secs: 30,
        });
    }

    #[test]
    fn test_acquire_response_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&AcquireResponse {
            lease: sample_lease(),
            verdict: sample_verdict(),
        });
    }

    #[test]
    fn test_lease_owner_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&LeaseOwner {
            session: sample_holder().session,
        });
    }

    #[test]
    fn test_heartbeat_response_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&HeartbeatResponse {
            state: LeaseState::Queued,
            expires_at_ms: 1_793_000_060_000,
            end_cause: None,
        });
        assert_json_round_trip(&HeartbeatResponse {
            state: LeaseState::Expired,
            expires_at_ms: 1_793_000_060_000,
            end_cause: Some(EndCause::Killed),
        });
    }

    #[test]
    fn test_heartbeat_response_from_an_older_allotd_has_no_cause() {
        let old: HeartbeatResponse =
            serde_json::from_str(r#"{"state":"expired","expires_at_ms":5}"#).unwrap();
        assert_eq!(old.end_cause, None);
    }

    #[test]
    fn test_release_response_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&ReleaseResponse {
            lease: sample_lease(),
            noise: None,
            run: None,
            next: None,
        });
        let profile = sample_lease().profile;
        assert_json_round_trip(&ReleaseResponse {
            lease: sample_lease(),
            noise: Some(NoiseReport {
                avg_cpu_pct: 1.5,
                peak_cpu_pct: 4.0,
                avg_gpu_pct: 0.0,
                peak_gpu_pct: 2.0,
                quiet_at_start: true,
            }),
            run: Some(RunReport {
                used: profile,
                counted: true,
            }),
            next: Some(NextReservation {
                profile,
                reasons: vec!["cores 2.2 from p90 of 3 past run(s) plus 10% (declared 8.0)".into()],
            }),
        });
    }

    #[test]
    fn test_release_response_from_an_older_allotd_has_no_run_or_next() {
        let mut json = serde_json::to_value(ReleaseResponse {
            lease: sample_lease(),
            noise: None,
            run: None,
            next: None,
        })
        .unwrap();
        assert!(
            json.get("run").is_none() && json.get("next").is_none(),
            "{json}"
        );
        json.as_object_mut().unwrap().remove("noise");
        let old: ReleaseResponse = serde_json::from_value(json).unwrap();
        assert_eq!((old.run, old.next), (None, None));
    }

    #[test]
    fn test_wait_request_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&WaitRequest {
            wait_timeout_secs: 90,
        });
    }

    #[test]
    fn test_lease_query_json_round_trip_preserves_empty_and_full_filters() {
        assert_json_round_trip(&LeaseQuery::default());
        assert_json_round_trip(&LeaseQuery {
            session: Some(sample_holder().session),
            state: Some(LeaseState::Active),
        });
    }

    #[test]
    fn test_lease_query_missing_fields_deserialize_as_none() {
        let query: LeaseQuery = serde_json::from_str("{}").unwrap();
        assert_eq!(query, LeaseQuery::default());
    }

    #[test]
    fn test_leases_response_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&LeasesResponse {
            leases: vec![sample_lease()],
        });
    }

    #[test]
    fn test_sessions_response_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&SessionsResponse {
            sessions: vec![sample_session_usage()],
        });
    }

    #[test]
    fn test_history_dtos_json_round_trip_preserve_every_field() {
        let ProfileSpec::Custom(p90) = custom_profile() else {
            panic!("the custom profile fixture is not custom");
        };
        let stats = RunStats {
            runs: 4,
            median: JobProfile {
                cores: 2.5,
                ram_mb: 900,
                ..p90
            },
            p90,
            max: JobProfile {
                est_secs: p90.est_secs * 2,
                ..p90
            },
        };
        let summary = JobSummary {
            fingerprint: r"cargo build|e:\work\app".into(),
            command: "cargo build".into(),
            cwd: r"E:\work\app".into(),
            runs: 5,
            estimate_runs: 4,
            last_ended_at_ms: 1_793_000_095_000,
            learned: Some(stats.p90),
            stats: Some(stats),
        };
        assert_json_round_trip(&HistoryResponse {
            jobs: vec![
                summary.clone(),
                JobSummary {
                    learned: None,
                    stats: None,
                    ..summary
                },
            ],
        });
        assert_json_round_trip(&HistoryRunsQuery {
            fingerprint: "cargo build|".into(),
            limit: Some(10),
        });
        assert_json_round_trip(&HistoryRunsResponse {
            runs: Vec::new(),
            stats: None,
        });
        assert_json_round_trip(&HistoryRunsResponse {
            runs: Vec::new(),
            stats: Some(stats),
        });
    }

    #[test]
    fn test_history_responses_from_an_older_allotd_have_no_stats() {
        let summary: JobSummary = serde_json::from_str(
            r#"{"fingerprint":"x|","command":"x","cwd":"","runs":1,"estimate_runs":0,
                "last_ended_at_ms":5,"learned":null}"#,
        )
        .unwrap();
        assert_eq!(summary.stats, None);
        let runs: HistoryRunsResponse = serde_json::from_str(r#"{"runs":[]}"#).unwrap();
        assert_eq!(runs.stats, None);
    }

    #[test]
    fn test_kill_and_remove_responses_json_round_trip_preserve_every_field() {
        assert_json_round_trip(&KillResponse {
            lease: sample_lease(),
            terminated: vec![KilledProcess {
                pid: 30_112,
                name: "bash.exe".into(),
            }],
            failed: vec![UnkilledProcess {
                pid: 30_140,
                name: "cargo.exe".into(),
                reason: "Access is denied.".into(),
            }],
            spared: vec![UnkilledProcess {
                pid: 30_150,
                name: "claude.exe".into(),
                reason: "protected".into(),
            }],
            untouched_reason: None,
        });
        let untouched = KillResponse {
            lease: sample_lease(),
            terminated: Vec::new(),
            failed: Vec::new(),
            spared: Vec::new(),
            untouched_reason: Some("the lease has no bound process tree".into()),
        };
        assert_json_round_trip(&untouched);
        assert_json_round_trip(&RemoveResponse {
            lease: sample_lease(),
        });
    }

    #[test]
    fn test_bind_request_without_command_omits_it() {
        let request = BindRequest {
            tool_use_id: "toolu_01AbC".into(),
            command: None,
        };
        assert_json_round_trip(&request);
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value, serde_json::json!({ "tool_use_id": "toolu_01AbC" }));
        assert_json_round_trip(&BindRequest {
            command: Some("cargo build".into()),
            ..request
        });
    }

    #[test]
    fn test_notice_dtos_json_round_trip_preserve_every_field() {
        assert_json_round_trip(&NoticesResponse {
            notices: Vec::new(),
        });
        assert_json_round_trip(&DeliveredResponse { delivered: true });
    }

    #[test]
    fn test_register_session_request_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&RegisterSessionRequest {
            holder: sample_holder(),
        });
    }
}
