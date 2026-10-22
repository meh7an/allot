use serde::{Deserialize, Serialize};

/// Identifies a lease. Assigned by the daemon's database when the lease is first stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LeaseId(pub i64);

/// Identifies a Claude Code session by its `CLAUDE_CODE_SESSION_ID`.
///
/// The MCP server reads the value from its environment and the hook reads `session_id` from
/// its stdin JSON; both see the same string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionKey(pub String);

/// The process a session's leases live and die with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holder {
    /// Session the leases belong to.
    pub session: SessionKey,
    /// PID of the session's `claude.exe`, the direct parent of its MCP server and its hooks.
    pub pid: u32,
    /// Start time of `pid` in seconds since the Unix epoch. Windows reuses PIDs, so a process
    /// with the same PID but another start time is not the holder.
    pub pid_start_time: u64,
}

/// One measurement averaged over the sampler's rolling windows.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rolling {
    /// Average over the last second.
    pub avg_1s: f32,
    /// Average over the last 10 seconds.
    pub avg_10s: f32,
    /// Average over the last 60 seconds.
    pub avg_60s: f32,
}

/// Machine-wide CPU load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CpuStats {
    /// Number of logical processors.
    pub logical_cores: u32,
    /// Total load as a percentage of all logical processors.
    pub usage_pct: Rolling,
    /// Load of each logical processor over the last second.
    pub per_core_pct: Vec<f32>,
}

/// Machine-wide physical memory and commit charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemStats {
    /// Installed physical memory.
    pub total_mb: u64,
    /// Physical memory in use.
    pub used_mb: u64,
    /// Physical memory available to new allocations without paging.
    pub available_mb: u64,
    /// Committed virtual memory.
    pub commit_used_mb: u64,
    /// Commit limit: physical memory plus page files.
    pub commit_limit_mb: u64,
}

/// Utilization of one GPU's engine groups, as percentages.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct GpuEngines {
    /// 3D (graphics) engines.
    pub three_d_pct: Rolling,
    /// Compute engines.
    pub compute_pct: Rolling,
    /// Copy engines.
    pub copy_pct: Rolling,
    /// Video encode and decode engines.
    pub video_pct: Rolling,
}

/// One GPU adapter as reported by the Windows PDH counters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpuAdapter {
    /// Adapter name, for example `NVIDIA GeForce RTX 4070`.
    pub name: String,
    /// Adapter LUID as it appears in PDH instance names, for example `0x00000000_0x0000d1a2`.
    pub luid: String,
    /// Engine utilization.
    pub engines: GpuEngines,
    /// Dedicated video memory in use.
    pub dedicated_used_mb: u64,
    /// Dedicated video memory installed, when the OS reports it.
    pub dedicated_total_mb: Option<u64>,
    /// Shared system memory in use by the adapter.
    pub shared_used_mb: u64,
}

/// Resources used by a process or a group of processes, in the units of [`JobProfile`].
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// CPU load in logical cores, for example `2.5` for two and a half busy cores.
    pub cores: f32,
    /// Resident memory.
    pub ram_mb: u64,
    /// GPU engine load as a percentage of one adapter.
    pub gpu_pct: f32,
    /// Dedicated video memory.
    pub vram_mb: u64,
}

/// Usage of a single process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessUsage {
    /// Process ID.
    pub pid: u32,
    /// Executable name, for example `cargo.exe`.
    pub name: String,
    /// Resources the process uses.
    pub usage: ResourceUsage,
}

/// Usage of a session's whole process subtree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionUsage {
    /// The session and its root process.
    pub holder: Holder,
    /// Sum over every process in the subtree.
    pub usage: ResourceUsage,
    /// The processes in the subtree, busiest first.
    pub processes: Vec<ProcessUsage>,
}

/// A point-in-time view of the machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// When the sample was taken, in milliseconds since the Unix epoch.
    pub taken_at_ms: i64,
    /// CPU load.
    pub cpu: CpuStats,
    /// Memory use.
    pub memory: MemStats,
    /// Every GPU adapter.
    pub gpus: Vec<GpuAdapter>,
    /// Usage of each registered session.
    pub sessions: Vec<SessionUsage>,
    /// Usage of processes outside every registered session.
    pub unattributed: ResourceUsage,
}

/// Resources a job is expected to need while it runs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct JobProfile {
    /// Logical cores the job keeps busy.
    pub cores: f32,
    /// Peak resident memory.
    pub ram_mb: u64,
    /// GPU engine load as a percentage of one adapter.
    pub gpu_pct: f32,
    /// Peak dedicated video memory.
    pub vram_mb: u64,
    /// Expected run time.
    pub est_secs: u64,
}

impl JobProfile {
    /// Returns why the profile is unusable, or `None` when every value is in range.
    pub fn invalid_reason(&self) -> Option<&'static str> {
        if !(self.cores.is_finite() && self.cores >= 0.0) {
            Some("cores must be a finite number of at least 0")
        } else if !(0.0..=100.0).contains(&self.gpu_pct) {
            Some("gpu_pct must be between 0 and 100")
        } else {
            None
        }
    }
}

/// Named job profiles. Their values come from the `[classes]` section of the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JobClass {
    /// Short jobs that barely load the machine.
    Light,
    /// Compiles, test suites, and other CPU-bound work.
    CpuHeavy,
    /// Training, rendering, games, and other GPU-bound work.
    GpuHeavy,
    /// Jobs that load both the CPU and the GPU.
    Mixed,
}

impl JobClass {
    /// Every class, in declaration order.
    pub const ALL: [Self; 4] = [Self::Light, Self::CpuHeavy, Self::GpuHeavy, Self::Mixed];

    /// The name used in config files and on the wire, for example `cpu_heavy`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::CpuHeavy => "cpu_heavy",
            Self::GpuHeavy => "gpu_heavy",
            Self::Mixed => "mixed",
        }
    }
}

/// How a lease shares the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LeaseKind {
    /// Runs alongside other leases as long as their reservations fit.
    Normal,
    /// Runs alone on a quiet machine so that measurements are not disturbed.
    BenchmarkExclusive,
}

/// Where a lease is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LeaseState {
    /// Waiting for resources.
    Queued,
    /// Granted; its reservation counts against the machine.
    Active,
    /// Released by its holder.
    Released,
    /// Ended by the daemon because the heartbeat stopped or the holder died.
    Expired,
}

/// A reservation of machine resources for one job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    /// Lease ID.
    pub id: LeaseId,
    /// Session that owns the lease.
    pub holder: Holder,
    /// How the lease shares the machine.
    pub kind: LeaseKind,
    /// Lifecycle state.
    pub state: LeaseState,
    /// Class the profile was taken from, or `None` for a custom profile.
    pub class: Option<JobClass>,
    /// Resources reserved while the lease is active.
    pub profile: JobProfile,
    /// What the lease is for, usually the command that will run.
    pub label: String,
    /// When the lease was requested, in milliseconds since the Unix epoch.
    pub requested_at_ms: i64,
    /// When the lease became active.
    pub granted_at_ms: Option<i64>,
    /// When the lease was released or expired.
    pub ended_at_ms: Option<i64>,
    /// When the lease expires unless a heartbeat arrives first.
    pub expires_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{assert_json_round_trip, sample_lease, sample_snapshot};

    fn profile_with(cores: f32, gpu_pct: f32) -> JobProfile {
        JobProfile {
            cores,
            ram_mb: 1024,
            gpu_pct,
            vram_mb: 0,
            est_secs: 60,
        }
    }

    #[test]
    fn test_snapshot_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&sample_snapshot());
    }

    #[test]
    fn test_lease_json_round_trip_preserves_every_field() {
        assert_json_round_trip(&sample_lease());
    }

    #[test]
    fn test_lease_json_uses_snake_case_enums_and_bare_ids() {
        let value = serde_json::to_value(sample_lease()).unwrap();
        assert_eq!(value["id"], 7);
        assert_eq!(value["holder"]["session"], "3f2a9c1e-session");
        assert_eq!(value["kind"], "benchmark_exclusive");
        assert_eq!(value["state"], "active");
        assert_eq!(value["class"], "cpu_heavy");
    }

    #[test]
    fn test_job_class_as_str_matches_serde_name() {
        for class in JobClass::ALL {
            let wire = serde_json::to_value(class).unwrap();
            assert_eq!(wire, class.as_str());
        }
    }

    #[test]
    fn test_job_profile_invalid_reason_in_range_is_none() {
        assert_eq!(profile_with(0.0, 0.0).invalid_reason(), None);
        assert_eq!(profile_with(16.0, 100.0).invalid_reason(), None);
    }

    #[test]
    fn test_job_profile_invalid_reason_out_of_range_names_field() {
        assert!(
            profile_with(-1.0, 0.0)
                .invalid_reason()
                .unwrap()
                .contains("cores")
        );
        assert!(
            profile_with(f32::NAN, 0.0)
                .invalid_reason()
                .unwrap()
                .contains("cores")
        );
        assert!(
            profile_with(1.0, 101.0)
                .invalid_reason()
                .unwrap()
                .contains("gpu_pct")
        );
        assert!(
            profile_with(1.0, f32::NAN)
                .invalid_reason()
                .unwrap()
                .contains("gpu_pct")
        );
    }
}
