// Generated from the Rust types behind contract/*.schema.json by
// shared/src/contract/swift.rs. Do not edit; rerun
// `ALLOT_UPDATE_CONTRACT=1 cargo test -p allot-shared contract` instead.

/// Response of `GET /ui/state`: everything the flyout shows, in one poll.
public struct UiState: Codable, Sendable, Equatable {
    /// Active leases, oldest grant first.
    public let active: [ActiveJob]

    /// Leases that ended in the last minute, however they ended, newest first, so the flyout
    /// can hold a card that just went away with how it ended. Older allotd builds send none.
    public let ended: [EndedLease]

    /// When a Claude session first registered with this install, or `nil` when none ever
    /// has, which shows the first-run card.
    public let firstSessionAtMs: Int64?

    /// When the state was built, in milliseconds since the Unix epoch. Elapsed and waited
    /// times are measured against it, so they do not depend on the tray app's clock.
    public let generatedAtMs: Int64

    /// Machine meters for the Resources card.
    public let meters: Meters

    /// Kills and removals, with whether Claude has been told yet.
    public let notices: [Notice]

    /// Queued leases, in queue order.
    public let queue: [QueuedJob]

    /// Recently finished jobs, newest first.
    public let recent: [RecentRun]

    /// The daemon itself.
    public let service: HealthResponse

    /// Every registered session, with or without leases.
    public let sessions: [UiSession]

    private enum CodingKeys: String, CodingKey {
        case active
        case ended
        case firstSessionAtMs = "first_session_at_ms"
        case generatedAtMs = "generated_at_ms"
        case meters
        case notices
        case queue
        case recent
        case service
        case sessions
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.active = try container.decode([ActiveJob].self, forKey: .active)
        self.ended = try container.decodeIfPresent([EndedLease].self, forKey: .ended) ?? []
        self.firstSessionAtMs = try container.decodeIfPresent(Int64.self, forKey: .firstSessionAtMs)
        self.generatedAtMs = try container.decode(Int64.self, forKey: .generatedAtMs)
        self.meters = try container.decode(Meters.self, forKey: .meters)
        self.notices = try container.decode([Notice].self, forKey: .notices)
        self.queue = try container.decode([QueuedJob].self, forKey: .queue)
        self.recent = try container.decode([RecentRun].self, forKey: .recent)
        self.service = try container.decode(HealthResponse.self, forKey: .service)
        self.sessions = try container.decode([UiSession].self, forKey: .sessions)
    }
}

/// Response of `POST /leases/{id}/kill`.
public struct KillResponse: Codable, Sendable, Equatable {
    /// Processes that could not be terminated, with what the OS said.
    public let failed: [UnkilledProcess]

    /// The lease, ended with cause `killed`.
    public let lease: Lease

    /// Processes in the tree left running on purpose, each with everything below it.
    public let spared: [UnkilledProcess]

    /// Processes terminated, root first.
    public let terminated: [KilledProcess]

    /// Why no process was terminated: the lease had no bound process tree, or its command had
    /// already ended. The lease ended anyway. `nil` when the bound tree was killed.
    public let untouchedReason: String?

    private enum CodingKeys: String, CodingKey {
        case failed
        case lease
        case spared
        case terminated
        case untouchedReason = "untouched_reason"
    }
}

/// Response of `POST /leases/{id}/remove`.
public struct RemoveResponse: Codable, Sendable, Equatable {
    /// The lease, ended with cause `removed` before it started.
    public let lease: Lease

    private enum CodingKeys: String, CodingKey {
        case lease
    }
}

/// How well a GPU source measures something per process.
public enum Accuracy: String, Codable, Sendable, Equatable {
    /// Measured per process.
    case exact
    /// Measured, but only roughly; the machine total is the reliable figure.
    case estimated
    /// Not measured: per-process values read 0.
    case unavailable
}

/// An active lease and the process its command runs as.
public struct ActiveJob: Codable, Sendable, Equatable {
    /// The lease.
    public let lease: Lease

    /// The command's process, or `nil` when the session has no process to show.
    public let process: JobProcess?

    /// For a benchmark lease, whether the machine was quiet when it was granted; `false` means
    /// it was granted anyway after the longest quiet wait. `nil` for other leases and when
    /// the daemon restarted since the grant.
    public let quietAtStart: Bool?

    private enum CodingKeys: String, CodingKey {
        case lease
        case process
        case quietAtStart = "quiet_at_start"
    }
}

/// Where a waiting benchmark lease stands, so its holder can report progress.
public enum BenchmarkProgress: Codable, Sendable, Equatable {
    /// Other leases are still active; nothing new starts until they end.
    case draining(Draining)
    /// The machine has drained and its background load is being averaged.
    case measuring(Measuring)
    /// The average is above the quiet limits. The lease is granted anyway, marked as not
    /// quiet, once `grant_anyway_in_secs` have passed.
    case noisy(Noisy)

    /// Other leases are still active; nothing new starts until they end.
    public struct Draining: Codable, Sendable, Equatable {
        /// Active leases left.
        public let activeLeases: UInt32

        private enum CodingKeys: String, CodingKey {
            case activeLeases = "active_leases"
        }
    }

    /// The machine has drained and its background load is being averaged.
    public struct Measuring: Codable, Sendable, Equatable {
        /// Average total CPU load so far, in percent.
        public let cpuAvgPct: Float

        /// Average load of the busiest GPU engine so far, in percent.
        public let gpuAvgPct: Float

        /// Seconds averaged so far.
        public let measuredSecs: UInt64

        /// Seconds the average must cover.
        public let neededSecs: UInt64

        private enum CodingKeys: String, CodingKey {
            case cpuAvgPct = "cpu_avg_pct"
            case gpuAvgPct = "gpu_avg_pct"
            case measuredSecs = "measured_secs"
            case neededSecs = "needed_secs"
        }
    }

    /// The average is above the quiet limits. The lease is granted anyway, marked as not
    /// quiet, once `grant_anyway_in_secs` have passed.
    public struct Noisy: Codable, Sendable, Equatable {
        /// Average total CPU load over the quiet window, in percent.
        public let cpuAvgPct: Float

        /// CPU limit of a quiet machine.
        public let cpuLimitPct: Float

        /// Average load of the busiest GPU engine over the quiet window, in percent.
        public let gpuAvgPct: Float

        /// GPU limit of a quiet machine.
        public let gpuLimitPct: Float

        /// Seconds until the lease is granted without quiet.
        public let grantAnywayInSecs: UInt64

        private enum CodingKeys: String, CodingKey {
            case cpuAvgPct = "cpu_avg_pct"
            case cpuLimitPct = "cpu_limit_pct"
            case gpuAvgPct = "gpu_avg_pct"
            case gpuLimitPct = "gpu_limit_pct"
            case grantAnywayInSecs = "grant_anyway_in_secs"
        }
    }

    private enum TagCodingKeys: String, CodingKey {
        case phase
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: TagCodingKeys.self)
        let tag = try container.decode(String.self, forKey: .phase)
        switch tag {
        case "draining": self = try .draining(Draining(from: decoder))
        case "measuring": self = try .measuring(Measuring(from: decoder))
        case "noisy": self = try .noisy(Noisy(from: decoder))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .phase,
                in: container,
                debugDescription: "unknown BenchmarkProgress phase \(tag)"
            )
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: TagCodingKeys.self)
        switch self {
        case .draining(let variant):
            try container.encode("draining", forKey: .phase)
            try variant.encode(to: encoder)
        case .measuring(let variant):
            try container.encode("measuring", forKey: .phase)
            try variant.encode(to: encoder)
        case .noisy(let variant):
            try container.encode("noisy", forKey: .phase)
            try variant.encode(to: encoder)
        }
    }
}

/// One process, identified across PID reuse.
public struct BoundProcess: Codable, Sendable, Equatable {
    /// Process ID.
    public let pid: UInt32

    /// Start time of `pid` in seconds since the Unix epoch, as in `Holder::pid_start_time`.
    public let startTime: UInt64

    private enum CodingKeys: String, CodingKey {
        case pid
        case startTime = "start_time"
    }
}

/// Processor load.
public struct CpuMeter: Codable, Sendable, Equatable {
    /// Recent once-a-second loads for the sparkline, oldest first, at most 24.
    public let historyPct: [Float]

    /// Number of logical processors.
    public let logicalCores: UInt32

    /// Logical cores reserved by active leases.
    public let reservedCores: Float

    /// Total load over the last second, as a percentage of all logical processors.
    public let usagePct: Float

    private enum CodingKeys: String, CodingKey {
        case historyPct = "history_pct"
        case logicalCores = "logical_cores"
        case reservedCores = "reserved_cores"
        case usagePct = "usage_pct"
    }
}

/// Space and activity of one fixed drive.
public struct DriveUsage: Codable, Sendable, Equatable {
    /// Drive letter with its colon on Windows, for example `C:`; mount point elsewhere, for
    /// example `/home`.
    public let name: String

    /// Bytes read per second over the last second.
    public let readBytesPerSec: UInt64

    /// Capacity of the volume.
    public let totalMb: UInt64

    /// Space in use.
    public let usedMb: UInt64

    /// Bytes written per second over the last second.
    public let writeBytesPerSec: UInt64

    private enum CodingKeys: String, CodingKey {
        case name
        case readBytesPerSec = "read_bytes_per_sec"
        case totalMb = "total_mb"
        case usedMb = "used_mb"
        case writeBytesPerSec = "write_bytes_per_sec"
    }
}

/// Why a lease ended.
///
/// Kept apart from `LeaseState`, whose values are fixed by the database's CHECK constraints
/// and by older `allot-mcp` builds that reject unknown states.
public enum EndCause: String, Codable, Sendable, Equatable {
    /// Its holder released it.
    case released
    /// The daemon ended it because the heartbeat stopped or the holder died.
    case expired
    /// The user stopped the job and its process tree.
    case killed
    /// The user removed the request from the queue before it started.
    case removed
}

/// A lease that ended a moment ago: released by its holder, ended by allotd, killed or removed.
public struct EndedLease: Codable, Sendable, Equatable {
    /// How it ended.
    public let cause: EndCause

    /// The lease's label, usually the command.
    public let command: String

    /// When it ended, in milliseconds since the Unix epoch.
    public let endedAtMs: Int64

    /// When it was granted, or `nil` for a request that never started.
    public let grantedAtMs: Int64?

    /// The lease.
    public let leaseId: Int64

    /// allotd's reason for the end, for example "no command running and the session idle for
    /// 180s"; empty when none was recorded.
    public let reason: String

    /// Session that held it.
    public let session: String

    private enum CodingKeys: String, CodingKey {
        case cause
        case command
        case endedAtMs = "ended_at_ms"
        case grantedAtMs = "granted_at_ms"
        case leaseId = "lease_id"
        case reason
        case session
    }
}

/// Caps the OS enforces on a lease's cgroup; each is `nil` when not enforced.
public struct EnforcedLimits: Codable, Sendable, Equatable {
    /// CPU time limit in logical cores.
    public let cpuCores: Float?

    /// Memory above which the OS throttles the command and reclaims its memory.
    public let memoryHighMb: UInt64?

    /// Memory at which the OS kills the command.
    public let memoryMaxMb: UInt64?

    private enum CodingKeys: String, CodingKey {
        case cpuCores = "cpu_cores"
        case memoryHighMb = "memory_high_mb"
        case memoryMaxMb = "memory_max_mb"
    }
}

/// One GPU adapter.
public struct GpuMeter: Codable, Sendable, Equatable {
    /// Load of the busiest engine group over the last second, in percent.
    public let busiestPct: Float

    /// Adapter name, for example `NVIDIA GeForce RTX 4070`.
    public let name: String

    /// How well each job's GPU load is measured on this adapter.
    public let processLoad: Accuracy

    /// Engine load reserved by active leases, in percent. Leases reserve on the adapter with
    /// the most video memory only, so it is 0 on every other adapter.
    public let reservedPct: Float

    /// Video memory reserved by active leases; 0 on every adapter but the reserving one. On a
    /// unified-memory adapter it comes out of RAM, on top of the RAM meter's reservation.
    public let reservedVramMb: UInt64

    /// Whether the adapter's video memory is the machine's RAM, as on Apple Silicon.
    public let unifiedMemory: Bool

    /// Dedicated video memory installed, when the OS reports it; `nil` on a unified-memory
    /// adapter.
    public let vramTotalMb: UInt64?

    /// Dedicated video memory in use; on a unified-memory adapter, the RAM the GPU has
    /// allocated.
    public let vramUsedMb: UInt64

    private enum CodingKeys: String, CodingKey {
        case busiestPct = "busiest_pct"
        case name
        case processLoad = "process_load"
        case reservedPct = "reserved_pct"
        case reservedVramMb = "reserved_vram_mb"
        case unifiedMemory = "unified_memory"
        case vramTotalMb = "vram_total_mb"
        case vramUsedMb = "vram_used_mb"
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.busiestPct = try container.decode(Float.self, forKey: .busiestPct)
        self.name = try container.decode(String.self, forKey: .name)
        self.processLoad =
            try container.decodeIfPresent(Accuracy.self, forKey: .processLoad) ?? .exact
        self.reservedPct = try container.decode(Float.self, forKey: .reservedPct)
        self.reservedVramMb = try container.decode(UInt64.self, forKey: .reservedVramMb)
        self.unifiedMemory =
            try container.decodeIfPresent(Bool.self, forKey: .unifiedMemory) ?? false
        self.vramTotalMb = try container.decodeIfPresent(UInt64.self, forKey: .vramTotalMb)
        self.vramUsedMb = try container.decode(UInt64.self, forKey: .vramUsedMb)
    }
}

/// Response of `GET /health`.
public struct HealthResponse: Codable, Sendable, Equatable {
    /// Why the last change to `config.toml` was rejected; the daemon keeps the config it had
    /// until the file is fixed.
    public let configError: String?

    /// Daemon process ID.
    public let pid: UInt32

    /// Seconds since the daemon started.
    public let uptimeSecs: UInt64

    /// Daemon version, from its `Cargo.toml`.
    public let version: String

    private enum CodingKeys: String, CodingKey {
        case configError = "config_error"
        case pid
        case uptimeSecs = "uptime_secs"
        case version
    }
}

/// The process a session's leases live and die with.
public struct Holder: Codable, Sendable, Equatable {
    /// PID of the session's Claude Code process (`claude.exe` on Windows, `claude` elsewhere),
    /// the direct parent of its MCP server and its hooks.
    public let pid: UInt32

    /// Start time of `pid` in seconds since the Unix epoch. Every OS reuses PIDs, so a process
    /// with the same PID but another start time is not the holder.
    public let pidStartTime: UInt64

    /// Session the leases belong to.
    public let session: String

    private enum CodingKeys: String, CodingKey {
        case pid
        case pidStartTime = "pid_start_time"
        case session
    }
}

/// How a lease's processes are found and stopped.
public enum IsolationKind: String, Codable, Sendable, Equatable {
    /// A cgroup of its own: exact usage, and a kill ends every process in it at once.
    case cgroup
    /// The process tree under the command's root, followed sample by sample.
    case tree
}

/// Named job profiles. Their values come from the `[classes]` section of the config.
public enum JobClass: String, Codable, Sendable, Equatable {
    /// Short jobs that barely load the machine.
    case light
    /// Compiles, test suites, and other CPU-bound work.
    case cpuHeavy = "cpu_heavy"
    /// Training, rendering, games, and other GPU-bound work.
    case gpuHeavy = "gpu_heavy"
    /// Jobs that load both the CPU and the GPU.
    case mixed
}

/// The process shown for a job.
public struct JobProcess: Codable, Sendable, Equatable {
    /// Whether it belongs to the command's bound process tree, the only tree a kill ends: its
    /// busiest process, or its root while the tree is idle. `false` means it is the session's
    /// busiest process, shown for display only.
    public let bound: Bool

    /// Full path of the executable, for its icon; `nil` when it could not be read.
    public let exe: String?

    /// Executable name, for example `cargo.exe`.
    public let name: String

    /// Process ID.
    public let pid: UInt32

    private enum CodingKeys: String, CodingKey {
        case bound
        case exe
        case name
        case pid
    }
}

/// Resources a job is expected to need while it runs.
public struct JobProfile: Codable, Sendable, Equatable {
    /// Logical cores the job keeps busy.
    public let cores: Float

    /// Expected run time.
    public let estSecs: UInt64

    /// GPU engine load as a percentage of one adapter.
    public let gpuPct: Float

    /// Peak resident memory.
    public let ramMb: UInt64

    /// Peak dedicated video memory.
    public let vramMb: UInt64

    private enum CodingKeys: String, CodingKey {
        case cores
        case estSecs = "est_secs"
        case gpuPct = "gpu_pct"
        case ramMb = "ram_mb"
        case vramMb = "vram_mb"
    }
}

/// A process a kill terminated.
public struct KilledProcess: Codable, Sendable, Equatable {
    /// Executable name, for example `cargo.exe`.
    public let name: String

    /// Process ID.
    public let pid: UInt32

    private enum CodingKeys: String, CodingKey {
        case name
        case pid
    }
}

/// A reservation of machine resources for one job.
public struct Lease: Codable, Sendable, Equatable {
    /// The command the lease's holder last reported running under it; `nil` until the hook
    /// reports one.
    public let binding: LeaseBinding?

    /// Class the profile was taken from, or `nil` for a custom profile.
    public let `class`: JobClass?

    /// Working directory of the command, or empty when the requester did not say.
    public let cwd: String

    /// Why the lease ended; `nil` while it is queued or active, and for leases that ended
    /// before causes were recorded.
    public let endCause: EndCause?

    /// When the lease was released or expired.
    public let endedAtMs: Int64?

    /// When the lease expires unless a heartbeat arrives first.
    public let expiresAtMs: Int64

    /// When the lease became active.
    public let grantedAtMs: Int64?

    /// Session that owns the lease.
    public let holder: Holder

    /// Lease ID.
    public let id: Int64

    /// How the lease's command is kept apart from the rest of the machine; `nil` where allotd
    /// does not report it, as on Windows.
    public let isolation: LeaseIsolation?

    /// How the lease shares the machine.
    public let kind: LeaseKind

    /// What the lease is for, usually the command that will run.
    public let label: String

    /// Resources reserved while the lease is active.
    public let profile: JobProfile

    /// When the lease was requested, in milliseconds since the Unix epoch.
    public let requestedAtMs: Int64

    /// Lifecycle state.
    public let state: LeaseState

    private enum CodingKeys: String, CodingKey {
        case binding
        case `class`
        case cwd
        case endCause = "end_cause"
        case endedAtMs = "ended_at_ms"
        case expiresAtMs = "expires_at_ms"
        case grantedAtMs = "granted_at_ms"
        case holder
        case id
        case isolation
        case kind
        case label
        case profile
        case requestedAtMs = "requested_at_ms"
        case state
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.binding = try container.decodeIfPresent(LeaseBinding.self, forKey: .binding)
        self.`class` = try container.decodeIfPresent(JobClass.self, forKey: .`class`)
        self.cwd = try container.decodeIfPresent(String.self, forKey: .cwd) ?? ""
        self.endCause = try container.decodeIfPresent(EndCause.self, forKey: .endCause)
        self.endedAtMs = try container.decodeIfPresent(Int64.self, forKey: .endedAtMs)
        self.expiresAtMs = try container.decode(Int64.self, forKey: .expiresAtMs)
        self.grantedAtMs = try container.decodeIfPresent(Int64.self, forKey: .grantedAtMs)
        self.holder = try container.decode(Holder.self, forKey: .holder)
        self.id = try container.decode(Int64.self, forKey: .id)
        self.isolation = try container.decodeIfPresent(LeaseIsolation.self, forKey: .isolation)
        self.kind = try container.decode(LeaseKind.self, forKey: .kind)
        self.label = try container.decode(String.self, forKey: .label)
        self.profile = try container.decode(JobProfile.self, forKey: .profile)
        self.requestedAtMs = try container.decode(Int64.self, forKey: .requestedAtMs)
        self.state = try container.decode(LeaseState.self, forKey: .state)
    }
}

/// The command a lease covers, as the PreToolUse hook reported it, and the process tree it
/// runs as once that is found. A later report for the same lease replaces it.
public struct LeaseBinding: Codable, Sendable, Equatable {
    /// When the hook reported the command, in milliseconds since the Unix epoch. Processes
    /// that started before it are never part of the command.
    public let boundAtMs: Int64

    /// Root of the command's process tree; `nil` until it is found.
    public let root: BoundProcess?

    /// Claude Code's ID for the tool call that runs the command.
    public let toolUseId: String

    private enum CodingKeys: String, CodingKey {
        case boundAtMs = "bound_at_ms"
        case root
        case toolUseId = "tool_use_id"
    }
}

/// How a lease's command is accounted for and stopped.
public struct LeaseIsolation: Codable, Sendable, Equatable {
    /// Whether the command runs in its own cgroup or is followed as a process tree.
    public let kind: IsolationKind

    /// Caps the OS enforces on the command.
    public let limits: EnforcedLimits

    /// Name of the systemd scope that holds the command, for example
    /// `allot-lease-42.scope`; `nil` without a cgroup.
    public let scope: String?

    private enum CodingKeys: String, CodingKey {
        case kind
        case limits
        case scope
    }
}

/// How a lease shares the machine.
public enum LeaseKind: String, Codable, Sendable, Equatable {
    /// Runs alongside other leases as long as their reservations fit.
    case normal
    /// Runs alone on a quiet machine so that measurements are not disturbed.
    case benchmarkExclusive = "benchmark_exclusive"
}

/// Where a lease is in its lifecycle.
public enum LeaseState: String, Codable, Sendable, Equatable {
    /// Waiting for resources.
    case queued
    /// Granted; its reservation counts against the machine.
    case active
    /// Released by its holder.
    case released
    /// Ended by the daemon because the heartbeat stopped or the holder died.
    case expired
}

/// Machine meters with the share that leases reserve.
public struct Meters: Codable, Sendable, Equatable {
    /// Processor load.
    public let cpu: CpuMeter

    /// Every fixed drive, by drive letter on Windows and by mount point elsewhere.
    public let drives: [DriveUsage]

    /// Every GPU adapter.
    public let gpus: [GpuMeter]

    /// Percentage of each resource that leases never take, from `admission.headroom_pct`.
    public let headroomPct: Float

    /// Physical memory.
    public let ram: RamMeter

    private enum CodingKeys: String, CodingKey {
        case cpu
        case drives
        case gpus
        case headroomPct = "headroom_pct"
        case ram
    }
}

/// Load from processes outside the holder's session while a benchmark lease was active.
///
/// CPU is the summed per-process load of every other session and of processes outside all
/// sessions. Kernel and interrupt time belongs to no process, so it is not counted: it would
/// otherwise charge the benchmark's own system time as noise.
public struct NoiseReport: Codable, Sendable, Equatable {
    /// Average CPU load of other processes, as a percentage of all logical processors.
    public let avgCpuPct: Float

    /// Average GPU engine load of other processes, as a percentage of one adapter.
    public let avgGpuPct: Float

    /// Peak CPU load of other processes.
    public let peakCpuPct: Float

    /// Peak GPU engine load of other processes.
    public let peakGpuPct: Float

    /// Whether the machine met the quiet limits when the lease was granted. `false` means it
    /// was granted anyway after the longest quiet wait.
    public let quietAtStart: Bool

    private enum CodingKeys: String, CodingKey {
        case avgCpuPct = "avg_cpu_pct"
        case avgGpuPct = "avg_gpu_pct"
        case peakCpuPct = "peak_cpu_pct"
        case peakGpuPct = "peak_gpu_pct"
        case quietAtStart = "quiet_at_start"
    }
}

/// A kill or removal that Claude must hear about once.
public struct Notice: Codable, Sendable, Equatable {
    /// What the user did: `EndCause::Killed` or `EndCause::Removed`.
    public let cause: EndCause

    /// The lease's label, usually the command.
    public let command: String

    /// When a hook or tool result told the session, or `nil` while it has not been told.
    public let deliveredAtMs: Int64?

    /// When the lease ended, in milliseconds since the Unix epoch.
    public let endedAtMs: Int64

    /// Lease the notice is about. A lease ends once, so this also identifies the notice.
    public let leaseId: Int64

    /// Session to tell.
    public let session: String

    /// Claude Code's ID for the tool call the lease was bound to, or `nil` when no hook
    /// reported one. A hook compares it with its own call to say "this command".
    public let toolUseId: String?

    private enum CodingKeys: String, CodingKey {
        case cause
        case command
        case deliveredAtMs = "delivered_at_ms"
        case endedAtMs = "ended_at_ms"
        case leaseId = "lease_id"
        case session
        case toolUseId = "tool_use_id"
    }
}

/// A queued lease and why it waits.
public struct QueuedJob: Codable, Sendable, Equatable {
    /// The lease.
    public let lease: Lease

    /// Place in the queue, starting at 1.
    public let position: UInt32

    /// Why it waits, the expected wait, and benchmark progress when it is a benchmark.
    public let verdict: Verdict

    private enum CodingKeys: String, CodingKey {
        case lease
        case position
        case verdict
    }
}

/// Physical memory.
public struct RamMeter: Codable, Sendable, Equatable {
    /// Memory reserved by active leases.
    public let reservedMb: UInt64

    /// Installed physical memory.
    public let totalMb: UInt64

    /// Physical memory in use.
    public let usedMb: UInt64

    private enum CodingKeys: String, CodingKey {
        case reservedMb = "reserved_mb"
        case totalMb = "total_mb"
        case usedMb = "used_mb"
    }
}

/// A finished job.
public struct RecentRun: Codable, Sendable, Equatable {
    /// The lease's label, usually the command as it was run.
    public let command: String

    /// Working directory of the command.
    public let cwd: String

    /// Profile the lease reserved.
    public let declared: JobProfile

    /// Why it ended; `nil` for runs recorded before causes were.
    public let endCause: EndCause?

    /// End of the run, in milliseconds since the Unix epoch.
    public let endedAtMs: Int64

    /// Lease the job ran under, if the lease is still stored.
    public let leaseId: Int64?

    /// Background load during the run; present only for benchmark leases.
    public let noise: NoiseReport?

    /// Session that ran it; `nil` for runs recorded before sessions were.
    public let session: String?

    /// Start of the run, in milliseconds since the Unix epoch.
    public let startedAtMs: Int64

    private enum CodingKeys: String, CodingKey {
        case command
        case cwd
        case declared
        case endCause = "end_cause"
        case endedAtMs = "ended_at_ms"
        case leaseId = "lease_id"
        case noise
        case session
        case startedAtMs = "started_at_ms"
    }
}

/// Resources used by a process or a group of processes, in the units of `JobProfile`.
public struct ResourceUsage: Codable, Sendable, Equatable {
    /// CPU load in logical cores, for example `2.5` for two and a half busy cores.
    public let cores: Float

    /// GPU engine load as a percentage of one adapter.
    public let gpuPct: Float

    /// Resident memory; on macOS the physical footprint, as Activity Monitor shows it.
    public let ramMb: UInt64

    /// Dedicated video memory.
    public let vramMb: UInt64

    private enum CodingKeys: String, CodingKey {
        case cores
        case gpuPct = "gpu_pct"
        case ramMb = "ram_mb"
        case vramMb = "vram_mb"
    }
}

/// One registered Claude session.
public struct UiSession: Codable, Sendable, Equatable {
    /// Working directory of the session's most recent lease, which names its project; `nil`
    /// when no lease gave one.
    public let cwd: String?

    /// Session ID.
    public let session: String

    /// Live usage of the session's whole process subtree.
    public let usage: ResourceUsage

    private enum CodingKeys: String, CodingKey {
        case cwd
        case session
        case usage
    }
}

/// A process in a killed tree that is still running.
public struct UnkilledProcess: Codable, Sendable, Equatable {
    /// Executable name, for example `claude.exe`.
    public let name: String

    /// Process ID.
    public let pid: UInt32

    /// Why it is still running.
    public let reason: String

    private enum CodingKeys: String, CodingKey {
        case name
        case pid
        case reason
    }
}

/// The admission engine's answer. Response of `POST /can-fit`.
public struct Verdict: Codable, Sendable, Equatable {
    /// Where a benchmark at the head of the queue stands; `nil` for other requests.
    public let benchmark: BenchmarkProgress?

    /// Whether the job could start now.
    public let fits: Bool

    /// Human-readable explanations, one per limiting or notable factor.
    public let reasons: [String]

    /// Expected wait before the job fits, when the daemon can estimate it.
    public let waitEstimateSecs: UInt64?

    private enum CodingKeys: String, CodingKey {
        case benchmark
        case fits
        case reasons
        case waitEstimateSecs = "wait_estimate_secs"
    }
}
