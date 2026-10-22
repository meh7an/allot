use serde::{Deserialize, Serialize};

use crate::types::{
    Holder, JobClass, JobProfile, Lease, LeaseKind, LeaseState, SessionKey, SessionUsage,
};

/// Response of `GET /health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Daemon version, from its `Cargo.toml`.
    pub version: String,
    /// Daemon process ID.
    pub pid: u32,
    /// Seconds since the daemon started.
    pub uptime_secs: u64,
}

/// How a request states the resources a job needs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProfileSpec {
    /// A named class, resolved through the daemon's config.
    Class(JobClass),
    /// Explicit values.
    Custom(JobProfile),
}

/// Body of `POST /can-fit`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanFitRequest {
    /// Resources the job needs.
    pub profile: ProfileSpec,
    /// How the job would share the machine.
    pub kind: LeaseKind,
}

/// The admission engine's answer. Response of `POST /can-fit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// Whether the job could start now.
    pub fits: bool,
    /// Human-readable explanations, one per limiting or notable factor.
    pub reasons: Vec<String>,
    /// Expected wait before the job fits, when the daemon can estimate it.
    pub wait_estimate_secs: Option<u64>,
}

/// Body of `POST /leases`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcquireRequest {
    /// Session asking for the lease.
    pub holder: Holder,
    /// Resources to reserve.
    pub profile: ProfileSpec,
    /// How the lease shares the machine.
    pub kind: LeaseKind,
    /// What the lease is for, usually the command that will run.
    pub label: String,
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

/// Query string of `POST /leases/{id}/heartbeat` and `DELETE /leases/{id}`.
///
/// The daemon refuses either call when the lease belongs to another session.
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
}

/// Response of `DELETE /leases/{id}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseResponse {
    /// The lease in its final state.
    pub lease: Lease,
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
        });
    }

    #[test]
    fn test_can_fit_request_json_round_trip_preserves_both_profile_forms() {
        assert_json_round_trip(&CanFitRequest {
            profile: ProfileSpec::Class(JobClass::GpuHeavy),
            kind: LeaseKind::Normal,
        });
        assert_json_round_trip(&CanFitRequest {
            profile: custom_profile(),
            kind: LeaseKind::BenchmarkExclusive,
        });
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
        });
    }

    #[test]
    fn test_acquire_request_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&AcquireRequest {
            holder: sample_holder(),
            profile: custom_profile(),
            kind: LeaseKind::Normal,
            label: "cargo build --release".into(),
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
        });
    }

    #[test]
    fn test_release_response_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&ReleaseResponse {
            lease: sample_lease(),
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
}
