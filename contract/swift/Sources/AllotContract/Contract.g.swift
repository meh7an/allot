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

/// Every allotd setting, page by page, in the order the Settings window shows them.
///
/// Panels add what holds no key themselves: the Panel page of their own settings, live
/// explanations, the command test, the service status, the config file row and Reset.
public struct SettingsSpec: Codable, Sendable, Equatable {
    /// The pages, in window order.
    public let pages: [SettingsPageSpec]

    private enum CodingKeys: String, CodingKey {
        case pages
    }
}

/// The JSON in `~/.allot/stopped.json`.
public struct StopRecord: Codable, Sendable, Equatable {
    /// Who stopped it.
    public let by: StoppedBy

    /// When allotd was stopped, in Unix milliseconds.
    public let stoppedAtMs: Int64

    private enum CodingKeys: String, CodingKey {
        case by
        case stoppedAtMs = "stopped_at_ms"
    }
}

/// Response of `GET /settings` and of a `PATCH /settings` that was written.
public struct SettingsState: Codable, Sendable, Equatable {
    /// Why allotd rejects `config.toml`, if it does. Settings cannot be changed until the
    /// file is fixed by hand.
    public let fileError: String?

    /// The port allotd listens on, which differs from `daemon.port` until a restart.
    public let listeningPort: UInt32

    /// Keys whose saved value takes effect only once allotd restarts, such as `daemon.port`.
    public let pendingRestart: [String]

    /// The revision of `config.toml`, to send with the next `PATCH /settings`.
    public let revision: String

    /// Every setting allotd owns, with its page, control, range, default, label and help.
    public let spec: SettingsSpec

    /// The value of every setting on the spec's pages: the file's, or while allotd rejects the
    /// file, the last good values, which stay in effect.
    public let values: [SettingValue]

    private enum CodingKeys: String, CodingKey {
        case fileError = "file_error"
        case listeningPort = "listening_port"
        case pendingRestart = "pending_restart"
        case revision
        case spec
        case values
    }
}

/// Body of `PATCH /settings`.
public struct SettingsPatch: Codable, Sendable, Equatable {
    /// The changes, written together or not at all. A reset is a change too.
    public let changes: [Change]

    /// The revision the changes were made against, from the last `GET /settings`.
    public let revision: String

    private enum CodingKeys: String, CodingKey {
        case changes
        case revision
    }
}

/// Body of `POST /settings/preview`.
public struct PreviewRequest: Codable, Sendable, Equatable {
    /// Unsaved changes, applied on top of the file; none previews the file as it is.
    public let changes: [Change]

    private enum CodingKeys: String, CodingKey {
        case changes
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.changes = try container.decodeIfPresent([Change].self, forKey: .changes) ?? []
    }
}

/// What fits on this machine with proposed settings.
public struct FitPreview: Codable, Sendable, Equatable {
    /// One per class, in `JobClass::ALL` order.
    public let classes: [SizeFit]

    /// The machine's CPU load, as the 10 s average the counts use.
    public let cpuBusyPct: Float

    /// What the proposed headroom keeps free.
    public let keptFree: KeptFree

    /// One per preset, by name.
    public let presets: [SizeFit]

    private enum CodingKeys: String, CodingKey {
        case classes
        case cpuBusyPct = "cpu_busy_pct"
        case keptFree = "kept_free"
        case presets
    }
}

/// Body of `POST /classify`.
public struct ClassifyRequest: Codable, Sendable, Equatable {
    /// Unsaved changes, such as edited rules, applied on top of the file.
    public let changes: [Change]

    /// The command line to test.
    public let line: String

    /// The shell the line is written for.
    public let shell: Shell

    private enum CodingKeys: String, CodingKey {
        case changes
        case line
        case shell
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.changes = try container.decodeIfPresent([Change].self, forKey: .changes) ?? []
        self.line = try container.decode(String.self, forKey: .line)
        self.shell = try container.decodeIfPresent(Shell.self, forKey: .shell) ?? .bash
    }
}

/// How the hook treats a command line under proposed rules.
public struct CommandTest: Codable, Sendable, Equatable {
    /// Every simple command of the line, in order.
    public let commands: [CommandResult]

    /// The one lease the hook asks for: the class of every heavy command combined. `nil`
    /// when no command is heavy.
    public let lease: ClassSize?

    private enum CodingKeys: String, CodingKey {
        case commands
        case lease
    }
}

/// Body of a refused settings request. Nothing was written.
public struct SettingsRefusal: Codable, Sendable, Equatable {
    /// What went wrong, as one sentence.
    public let message: String

    /// Every rejected value with its key, when values were the cause; empty otherwise.
    public let problems: [FieldError]

    /// The file's revision now, when it changed since the request's revision was read.
    public let revision: String?

    private enum CodingKeys: String, CodingKey {
        case message
        case problems
        case revision
    }
}

/// Body of `POST /shutdown` from a panel's Stop service. allotd writes the stop record before
/// it stops, so no session starts it again; a request with no body stops it without one.
public struct ShutdownRequest: Codable, Sendable, Equatable {
    /// Who stopped it, for the record.
    public let by: StoppedBy

    private enum CodingKeys: String, CodingKey {
        case by
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

/// One end of a number's range.
public struct Bound: Codable, Sendable, Equatable {
    /// Whether the limit itself is out of range.
    public let exclusive: Bool

    /// The limit.
    public let value: Double

    private enum CodingKeys: String, CodingKey {
        case exclusive
        case value
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

/// One change to `config.toml`. Keys are the spec's dotted keys, such as
/// `admission.headroom_pct`; tables are a size table's key, `classes` or `presets`.
public enum Change: Codable, Sendable, Equatable {
    /// Turns a switch on or off.
    case setSwitch(SetSwitch)
    /// Sets a number, in the unit of its setting. A whole number is written as an integer
    /// for whole-number settings and as a float, such as `8.0`, for the others.
    case setNumber(SetNumber)
    /// Replaces a list of strings, such as `classifier.light_flags`, as a whole.
    case setList(SetList)
    /// Replaces `classifier.rules` as a whole. An empty list is written as `rules = []`, so
    /// no rule applies; `Change::Reset` of `classifier.rules` brings back the built-in ones.
    case setRules(SetRules)
    /// Sets every field of a class or preset, adding the preset when it is new. A field is
    /// written when the file already has it or it differs from the row's default.
    case setRow(SetRow)
    /// Removes a class or preset from the file: a built-in row takes its defaults again, and
    /// a preset of the user's own is deleted. A rename is this and a `Change::SetRow`.
    case resetRow(ResetRow)
    /// Removes a setting from the file so its default applies, or with a size table's key,
    /// every row of it.
    case reset(Reset)

    /// Turns a switch on or off.
    public struct SetSwitch: Codable, Sendable, Equatable {
        /// The switch's key.
        public let key: String

        /// The new state.
        public let value: Bool

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    /// Sets a number, in the unit of its setting. A whole number is written as an integer
    /// for whole-number settings and as a float, such as `8.0`, for the others.
    public struct SetNumber: Codable, Sendable, Equatable {
        /// The number's key.
        public let key: String

        /// The new value.
        public let value: Double

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    /// Replaces a list of strings, such as `classifier.light_flags`, as a whole.
    public struct SetList: Codable, Sendable, Equatable {
        /// The list's key.
        public let key: String

        /// The new list.
        public let value: [String]

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    /// Replaces `classifier.rules` as a whole. An empty list is written as `rules = []`, so
    /// no rule applies; `Change::Reset` of `classifier.rules` brings back the built-in ones.
    public struct SetRules: Codable, Sendable, Equatable {
        /// The rules, in matching order.
        public let rules: [HeavyRule]

        private enum CodingKeys: String, CodingKey {
            case rules
        }
    }

    /// Sets every field of a class or preset, adding the preset when it is new. A field is
    /// written when the file already has it or it differs from the row's default.
    public struct SetRow: Codable, Sendable, Equatable {
        /// The fewest cores a lease of it may start with; `nil` leaves it automatic.
        public let minCores: Float?

        /// The row, such as `cpu_heavy` or a preset's name.
        public let name: String

        /// The row's values.
        public let profile: JobProfile

        /// The size table, `classes` or `presets`.
        public let table: String

        private enum CodingKeys: String, CodingKey {
            case minCores = "min_cores"
            case name
            case profile
            case table
        }
    }

    /// Removes a class or preset from the file: a built-in row takes its defaults again, and
    /// a preset of the user's own is deleted. A rename is this and a `Change::SetRow`.
    public struct ResetRow: Codable, Sendable, Equatable {
        /// The row.
        public let name: String

        /// The size table, `classes` or `presets`.
        public let table: String

        private enum CodingKeys: String, CodingKey {
            case name
            case table
        }
    }

    /// Removes a setting from the file so its default applies, or with a size table's key,
    /// every row of it.
    public struct Reset: Codable, Sendable, Equatable {
        /// The setting's key.
        public let key: String

        private enum CodingKeys: String, CodingKey {
            case key
        }
    }

    private enum TagCodingKeys: String, CodingKey {
        case kind
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: TagCodingKeys.self)
        let tag = try container.decode(String.self, forKey: .kind)
        switch tag {
        case "set_switch": self = try .setSwitch(SetSwitch(from: decoder))
        case "set_number": self = try .setNumber(SetNumber(from: decoder))
        case "set_list": self = try .setList(SetList(from: decoder))
        case "set_rules": self = try .setRules(SetRules(from: decoder))
        case "set_row": self = try .setRow(SetRow(from: decoder))
        case "reset_row": self = try .resetRow(ResetRow(from: decoder))
        case "reset": self = try .reset(Reset(from: decoder))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown Change kind \(tag)"
            )
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: TagCodingKeys.self)
        switch self {
        case .setSwitch(let variant):
            try container.encode("set_switch", forKey: .kind)
            try variant.encode(to: encoder)
        case .setNumber(let variant):
            try container.encode("set_number", forKey: .kind)
            try variant.encode(to: encoder)
        case .setList(let variant):
            try container.encode("set_list", forKey: .kind)
            try variant.encode(to: encoder)
        case .setRules(let variant):
            try container.encode("set_rules", forKey: .kind)
            try variant.encode(to: encoder)
        case .setRow(let variant):
            try container.encode("set_row", forKey: .kind)
            try variant.encode(to: encoder)
        case .resetRow(let variant):
            try container.encode("reset_row", forKey: .kind)
            try variant.encode(to: encoder)
        case .reset(let variant):
            try container.encode("reset", forKey: .kind)
            try variant.encode(to: encoder)
        }
    }
}

/// One value of a choice.
public struct ChoiceOption: Codable, Sendable, Equatable {
    /// The value as the panel shows it.
    public let label: String

    /// The value written to the file.
    public let value: String

    private enum CodingKeys: String, CodingKey {
        case label
        case value
    }
}

/// A class with the size the proposed settings give it.
public struct ClassSize: Codable, Sendable, Equatable {
    /// The lease class.
    public let `class`: JobClass

    /// The fewest cores a lease of it starts with: its own minimum, or the automatic one.
    public let minCores: Float

    /// What a lease of it asks for.
    public let profile: JobProfile

    private enum CodingKeys: String, CodingKey {
        case `class`
        case minCores = "min_cores"
        case profile
    }
}

/// How the rules treat one simple command.
public enum CommandKind: String, Codable, Sendable, Equatable {
    /// A rule matches, so it needs a lease.
    case heavy
    /// A rule matches, but a light flag makes it only print and exit.
    case printOnly = "print_only"
    /// No rule matches.
    case notHeavy = "not_heavy"
}

/// One simple command of a tested line.
public struct CommandResult: Codable, Sendable, Equatable {
    /// Its words joined by spaces, wrappers such as `sudo` included.
    public let command: String

    /// The first light flag among its arguments, for a print-only command.
    public let flag: String?

    /// How the proposed rules treat it.
    public let kind: CommandKind

    /// Position of the first matching rule, from 0, for a heavy command.
    public let ruleIndex: UInt32?

    /// The lease a heavy command needs on its own; `nil` for the others.
    public let size: ClassSize?

    private enum CodingKeys: String, CodingKey {
        case command
        case flag
        case kind
        case ruleIndex = "rule_index"
        case size
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

/// A value of the configuration outside what it allows: its key and what it must be.
public struct FieldError: Codable, Sendable, Equatable {
    /// The value's dotted key, such as `admission.headroom_pct`, `presets.big.min_cores` or
    /// `classifier.rules[2].program`. A preset's name is reported as `presets.<name>`.
    public let key: String

    /// What the value must be, such as `must be at least 0 and below 100`.
    public let message: String

    private enum CodingKeys: String, CodingKey {
        case key
        case message
    }
}

/// How many jobs of one size start, one after another, in a given room.
public struct Fits: Codable, Sendable, Equatable {
    /// Jobs that get every core they ask for.
    public let full: UInt32

    /// Whether more would still fit after the count's limit of 99 jobs.
    public let more: Bool

    /// Jobs after those that start shrunk, each counted at its minimum cores.
    public let shrunk: UInt32

    private enum CodingKeys: String, CodingKey {
        case full
        case more
        case shrunk
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

/// Marks a simple command as heavy when its program and arguments match.
///
/// Matching ignores case, and `*` matches any run of characters.
public struct HeavyRule: Codable, Sendable, Equatable {
    /// Any one of these makes the command heavy; empty matches any arguments. A pattern that
    /// starts with `-` matches that flag anywhere in the command; any other pattern matches
    /// the first positional argument (the first one not starting with `-` or `+`).
    public let args: [String]

    /// Class the matched command needs a lease for.
    public let `class`: JobClass

    /// Program name without directory or extension, for example `cargo` or `python*`.
    public let program: String

    private enum CodingKeys: String, CodingKey {
        case args
        case `class`
        case program
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.args = try container.decodeIfPresent([String].self, forKey: .args) ?? []
        self.`class` = try container.decode(JobClass.self, forKey: .`class`)
        self.program = try container.decode(String.self, forKey: .program)
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

/// What the headroom keeps free of the whole machine.
public struct KeptFree: Codable, Sendable, Equatable {
    /// Logical cores kept free.
    public let cores: Double

    /// Share of the primary GPU kept free, or `nil` without a GPU.
    public let gpuPct: Double?

    /// Memory kept free.
    public let ramMb: Double

    /// The machine's logical cores.
    public let totalCores: UInt32

    /// The machine's memory.
    public let totalRamMb: UInt64

    /// The primary GPU's video memory, under the same conditions.
    public let totalVramMb: UInt64?

    /// Video memory kept free, or `nil` when there is no GPU, its size is unknown, or it
    /// shares RAM.
    public let vramMb: Double?

    private enum CodingKeys: String, CodingKey {
        case cores
        case gpuPct = "gpu_pct"
        case ramMb = "ram_mb"
        case totalCores = "total_cores"
        case totalRamMb = "total_ram_mb"
        case totalVramMb = "total_vram_mb"
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
    /// Cores the request asked for, after history raised them; `nil` for leases from before
    /// minimums were recorded, which asked for `profile.cores`.
    public let askedCores: Float?

    /// The command the lease's holder last reported running under it; `nil` until the hook
    /// reports one.
    public let binding: LeaseBinding?

    /// Class the profile was taken from, or `nil` for a custom profile.
    public let `class`: JobClass?

    /// Working directory of the command, or empty when the requester did not say.
    public let cwd: String

    /// Cores the request declared before history lowered them; `nil` when history did not
    /// lower them. A lowered lease counts at these cores for its first seconds, before its
    /// job's load shows in the 10 s averages.
    public let declaredCores: Float?

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

    /// The fewest cores the lease may start with; `nil` for leases from before minimums were
    /// recorded, which may not shrink.
    public let minCores: Float?

    /// Resources reserved while the lease is active. Its cores are what the lease was granted,
    /// which is fewer than `asked_cores` when the grant shrank to fit.
    public let profile: JobProfile

    /// When the lease was requested, in milliseconds since the Unix epoch.
    public let requestedAtMs: Int64

    /// Lifecycle state.
    public let state: LeaseState

    private enum CodingKeys: String, CodingKey {
        case askedCores = "asked_cores"
        case binding
        case `class`
        case cwd
        case declaredCores = "declared_cores"
        case endCause = "end_cause"
        case endedAtMs = "ended_at_ms"
        case expiresAtMs = "expires_at_ms"
        case grantedAtMs = "granted_at_ms"
        case holder
        case id
        case isolation
        case kind
        case label
        case minCores = "min_cores"
        case profile
        case requestedAtMs = "requested_at_ms"
        case state
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.askedCores = try container.decodeIfPresent(Float.self, forKey: .askedCores)
        self.binding = try container.decodeIfPresent(LeaseBinding.self, forKey: .binding)
        self.`class` = try container.decodeIfPresent(JobClass.self, forKey: .`class`)
        self.cwd = try container.decodeIfPresent(String.self, forKey: .cwd) ?? ""
        self.declaredCores = try container.decodeIfPresent(Float.self, forKey: .declaredCores)
        self.endCause = try container.decodeIfPresent(EndCause.self, forKey: .endCause)
        self.endedAtMs = try container.decodeIfPresent(Int64.self, forKey: .endedAtMs)
        self.expiresAtMs = try container.decode(Int64.self, forKey: .expiresAtMs)
        self.grantedAtMs = try container.decodeIfPresent(Int64.self, forKey: .grantedAtMs)
        self.holder = try container.decode(Holder.self, forKey: .holder)
        self.id = try container.decode(Int64.self, forKey: .id)
        self.isolation = try container.decodeIfPresent(LeaseIsolation.self, forKey: .isolation)
        self.kind = try container.decode(LeaseKind.self, forKey: .kind)
        self.label = try container.decode(String.self, forKey: .label)
        self.minCores = try container.decodeIfPresent(Float.self, forKey: .minCores)
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

/// A switch that turns a numeric feature off by writing one value.
public struct OffSwitch: Codable, Sendable, Equatable {
    /// The switch's help.
    public let help: String

    /// The switch's label, describing the on state.
    public let label: String

    /// The value written while the switch is off.
    public let value: Double

    private enum CodingKeys: String, CodingKey {
        case help
        case label
        case value
    }
}

/// An operating system a panel runs on.
public enum Platform: String, Codable, Sendable, Equatable {
    /// The Windows tray.
    case windows
    /// The macOS menu bar app.
    case macos
    /// The GNOME extension and the KDE widget.
    case linux
}

/// What a number counts.
public enum Quantity: String, Codable, Sendable, Equatable {
    /// A share from 0 to 100.
    case percent
    /// A number of job runs.
    case runs
    /// A number of days.
    case days
    /// A duration in seconds; panels may show it in minutes or hours.
    case seconds
    /// Logical cores; fractions are allowed.
    case cores
    /// Memory in megabytes; panels may show it in gigabytes.
    case megabytes
    /// A TCP port, shown without digit grouping.
    case port
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

    /// Median, p90 and max of the job's runs, as `allot history` shows them; `nil` when no run
    /// of the job counts, and from older daemons.
    public let stats: RunStats?

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
        case stats
    }
}

/// A switch another setting depends on.
public struct Requirement: Codable, Sendable, Equatable {
    /// The switch's key.
    public let key: String

    /// Shown under the disabled setting while the switch is off.
    public let note: String

    private enum CodingKeys: String, CodingKey {
        case key
        case note
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

/// The name field of a row the user adds or renames.
public struct RowName: Codable, Sendable, Equatable {
    /// The field's help.
    public let help: String

    /// The field's label.
    public let label: String

    /// A regular expression the whole name must match. A name must also differ from every
    /// other row's and from every class's.
    public let pattern: String

    private enum CodingKeys: String, CodingKey {
        case help
        case label
        case pattern
    }
}

/// Median, p90 and max of what a job's runs used: the newest runs that neither overlapped
/// another lease of their session nor were killed, at most as many as an estimate reads.
/// Cores and GPU load are each run's average while its session was busy, RAM and VRAM its
/// peak, and `est_secs` its run time.
public struct RunStats: Codable, Sendable, Equatable {
    /// Highest of each value.
    public let max: JobProfile

    /// Nearest-rank median of each value.
    public let median: JobProfile

    /// Nearest-rank 90th percentile of each value, which estimates raise and lower to.
    public let p90: JobProfile

    /// Runs the statistics come from.
    public let runs: UInt32

    private enum CodingKeys: String, CodingKey {
        case max
        case median
        case p90
        case runs
    }
}

/// How a setting is edited.
public enum SettingControl: Codable, Sendable, Equatable {
    /// On or off.
    case `switch`(Switch)
    /// A number in a unit, written to the file in that unit.
    case number(Number)
    /// A line of text, never blank.
    case text(Text)
    /// One of a fixed set of values.
    case choice(Choice)
    /// A list of strings without blank entries; a list in the file replaces the default list.
    case stringList(StringList)
    /// Named rows of sizes, such as `[classes.cpu_heavy]`, edited one row at a time. A row
    /// in the file changes only the fields it names.
    case sizeTable(SizeTable)
    /// An ordered list of heavy-command rules, always written whole. A list in the file, even
    /// an empty one, replaces the default list; removing the key brings the default back.
    case ruleList(RuleList)

    /// On or off.
    public struct Switch: Codable, Sendable, Equatable {
        /// The value while the key is not in the file.
        public let `default`: Bool

        private enum CodingKeys: String, CodingKey {
            case `default`
        }
    }

    /// A number in a unit, written to the file in that unit.
    public struct Number: Codable, Sendable, Equatable {
        /// How the value is worked out while the key is left out, when it may be, shown
        /// beside an Automatic choice.
        public let automatic: String?

        /// The value while the key is not in the file. In a table's fields, the value a new
        /// row starts with, or `nil` when a new row must give one or `automatic` applies.
        public let `default`: Double?

        /// Whether only whole numbers are allowed.
        public let integer: Bool

        /// The largest value allowed, if any besides `max_key`.
        public let max: Bound?

        /// A setting this value must be at most, by key; in a table, a field of the same row.
        public let maxKey: String?

        /// The smallest value allowed, if any besides `min_key`.
        public let min: Bound?

        /// A setting this value must be at least, by key; in a table, a field of the same row.
        public let minKey: String?

        /// A switch in front of the number, when one value turns the feature off.
        public let off: OffSwitch?

        /// What the number counts, which picks its unit and display.
        public let quantity: Quantity

        private enum CodingKeys: String, CodingKey {
            case automatic
            case `default`
            case integer
            case max
            case maxKey = "max_key"
            case min
            case minKey = "min_key"
            case off
            case quantity
        }
    }

    /// A line of text, never blank.
    public struct Text: Codable, Sendable, Equatable {
        /// A regular expression the whole text must match, if any.
        public let pattern: String?

        private enum CodingKeys: String, CodingKey {
            case pattern
        }
    }

    /// One of a fixed set of values.
    public struct Choice: Codable, Sendable, Equatable {
        /// The value a new item starts with, or `nil` when one must be picked.
        public let `default`: String?

        /// The values in display order.
        public let options: [ChoiceOption]

        private enum CodingKeys: String, CodingKey {
            case `default`
            case options
        }
    }

    /// A list of strings without blank entries; a list in the file replaces the default list.
    public struct StringList: Codable, Sendable, Equatable {
        /// The list while the key is not in the file.
        public let `default`: [String]

        private enum CodingKeys: String, CodingKey {
            case `default`
        }
    }

    /// Named rows of sizes, such as `[classes.cpu_heavy]`, edited one row at a time. A row
    /// in the file changes only the fields it names.
    public struct SizeTable: Codable, Sendable, Equatable {
        /// The fields of every row.
        public let fields: [SettingSpec]

        /// How a row added by the user is named, or `nil` when rows cannot be added,
        /// renamed or deleted.
        public let newRowName: RowName?

        /// The built-in rows with their defaults, which always exist.
        public let rows: [SizeRowSpec]

        private enum CodingKeys: String, CodingKey {
            case fields
            case newRowName = "new_row_name"
            case rows
        }
    }

    /// An ordered list of heavy-command rules, always written whole. A list in the file, even
    /// an empty one, replaces the default list; removing the key brings the default back.
    public struct RuleList: Codable, Sendable, Equatable {
        /// The list while the key is not in the file.
        public let `default`: [HeavyRule]

        /// The fields of every rule.
        public let fields: [SettingSpec]

        private enum CodingKeys: String, CodingKey {
            case `default`
            case fields
        }
    }

    private enum TagCodingKeys: String, CodingKey {
        case kind
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: TagCodingKeys.self)
        let tag = try container.decode(String.self, forKey: .kind)
        switch tag {
        case "switch": self = try .`switch`(Switch(from: decoder))
        case "number": self = try .number(Number(from: decoder))
        case "text": self = try .text(Text(from: decoder))
        case "choice": self = try .choice(Choice(from: decoder))
        case "string_list": self = try .stringList(StringList(from: decoder))
        case "size_table": self = try .sizeTable(SizeTable(from: decoder))
        case "rule_list": self = try .ruleList(RuleList(from: decoder))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown SettingControl kind \(tag)"
            )
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: TagCodingKeys.self)
        switch self {
        case .`switch`(let variant):
            try container.encode("switch", forKey: .kind)
            try variant.encode(to: encoder)
        case .number(let variant):
            try container.encode("number", forKey: .kind)
            try variant.encode(to: encoder)
        case .text(let variant):
            try container.encode("text", forKey: .kind)
            try variant.encode(to: encoder)
        case .choice(let variant):
            try container.encode("choice", forKey: .kind)
            try variant.encode(to: encoder)
        case .stringList(let variant):
            try container.encode("string_list", forKey: .kind)
            try variant.encode(to: encoder)
        case .sizeTable(let variant):
            try container.encode("size_table", forKey: .kind)
            try variant.encode(to: encoder)
        case .ruleList(let variant):
            try container.encode("rule_list", forKey: .kind)
            try variant.encode(to: encoder)
        }
    }
}

/// One setting: a key of `config.toml`, or a field of a table row or rule.
public struct SettingSpec: Codable, Sendable, Equatable {
    /// Whether the setting is rarely changed and shows last on its page, under Advanced.
    public let advanced: Bool

    /// How the value is edited, with its default and range.
    public let control: SettingControl

    /// One or two plain sentences. Panels show the key after them.
    public let help: String

    /// The TOML key: a dotted path such as `admission.headroom_pct` for a setting on a page,
    /// or the key inside a row, such as `cores`, for a field of a table or rule.
    public let key: String

    /// A noun phrase in sentence case; a switch's label describes the on state.
    public let label: String

    /// Whether a change takes effect only once allotd restarts.
    public let needsRestart: Bool

    /// The operating systems whose panels show the setting; empty means every one.
    public let platforms: [Platform]

    /// A switch that must be on for this setting to act; panels disable it while it is off.
    public let requires: Requirement?

    private enum CodingKeys: String, CodingKey {
        case advanced
        case control
        case help
        case key
        case label
        case needsRestart = "needs_restart"
        case platforms
        case requires
    }
}

/// The value of one setting on a page, keyed like the spec, in the kind its control edits.
///
/// The tags end in `_value`, so the panels' generated types never share a name with
/// `SettingControl`'s: System.Text.Json's source generator warns on a repeated type name.
public enum SettingValue: Codable, Sendable, Equatable {
    /// The state of a switch.
    case switchValue(SwitchValue)
    /// A number, in the unit of its setting.
    case numberValue(NumberValue)
    /// A list of strings, such as `classifier.light_flags`.
    case listValue(ListValue)
    /// The heavy-command rules.
    case rulesValue(RulesValue)
    /// Every row of a size table, built-in rows included.
    case rowsValue(RowsValue)

    /// The state of a switch.
    public struct SwitchValue: Codable, Sendable, Equatable {
        /// The setting's key.
        public let key: String

        /// Whether it is on.
        public let value: Bool

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    /// A number, in the unit of its setting.
    public struct NumberValue: Codable, Sendable, Equatable {
        /// The setting's key.
        public let key: String

        /// The value.
        public let value: Double

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    /// A list of strings, such as `classifier.light_flags`.
    public struct ListValue: Codable, Sendable, Equatable {
        /// The setting's key.
        public let key: String

        /// The entries, in file order.
        public let value: [String]

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    /// The heavy-command rules.
    public struct RulesValue: Codable, Sendable, Equatable {
        /// The setting's key, `classifier.rules`.
        public let key: String

        /// The rules, in matching order.
        public let value: [HeavyRule]

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    /// Every row of a size table, built-in rows included.
    public struct RowsValue: Codable, Sendable, Equatable {
        /// The table's key, `classes` or `presets`.
        public let key: String

        /// The rows: classes in class order, presets by name.
        public let value: [SizeRow]

        private enum CodingKeys: String, CodingKey {
            case key
            case value
        }
    }

    private enum TagCodingKeys: String, CodingKey {
        case kind
    }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.container(keyedBy: TagCodingKeys.self)
        let tag = try container.decode(String.self, forKey: .kind)
        switch tag {
        case "switch_value": self = try .switchValue(SwitchValue(from: decoder))
        case "number_value": self = try .numberValue(NumberValue(from: decoder))
        case "list_value": self = try .listValue(ListValue(from: decoder))
        case "rules_value": self = try .rulesValue(RulesValue(from: decoder))
        case "rows_value": self = try .rowsValue(RowsValue(from: decoder))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown SettingValue kind \(tag)"
            )
        }
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.container(keyedBy: TagCodingKeys.self)
        switch self {
        case .switchValue(let variant):
            try container.encode("switch_value", forKey: .kind)
            try variant.encode(to: encoder)
        case .numberValue(let variant):
            try container.encode("number_value", forKey: .kind)
            try variant.encode(to: encoder)
        case .listValue(let variant):
            try container.encode("list_value", forKey: .kind)
            try variant.encode(to: encoder)
        case .rulesValue(let variant):
            try container.encode("rules_value", forKey: .kind)
            try variant.encode(to: encoder)
        case .rowsValue(let variant):
            try container.encode("rows_value", forKey: .kind)
            try variant.encode(to: encoder)
        }
    }
}

/// A titled group of settings on a page.
public struct SettingsGroupSpec: Codable, Sendable, Equatable {
    /// Stable identifier, unique on its page, such as `learn`.
    public let id: String

    /// The group's title.
    public let label: String

    /// The settings, in group order.
    public let settings: [SettingSpec]

    private enum CodingKeys: String, CodingKey {
        case id
        case label
        case settings
    }
}

/// A page of the Settings window.
public struct SettingsPageSpec: Codable, Sendable, Equatable {
    /// The groups, in page order.
    public let groups: [SettingsGroupSpec]

    /// Stable identifier, such as `admission`.
    public let id: String

    /// A sentence under the title, if the page has one.
    public let intro: String?

    /// The page's title.
    public let label: String

    private enum CodingKeys: String, CodingKey {
        case groups
        case id
        case intro
        case label
    }
}

/// The shell a command line is written for. Quoting and escapes differ between them.
public enum Shell: String, Codable, Sendable, Equatable {
    /// bash and other POSIX shells: `\` escapes.
    case bash
    /// PowerShell, also used for `cmd /c` scripts: `` ` `` escapes and `\` is literal.
    case powershell
}

/// What fits of one class or preset.
public struct SizeFit: Codable, Sendable, Equatable {
    /// With every lease ended; load outside leases stays.
    public let idle: Fits

    /// The fewest cores a job of it starts with.
    public let minCores: Float

    /// The class's key, such as `cpu_heavy`, or the preset's name.
    public let name: String

    /// Beside the active leases and the load right now.
    public let now: Fits

    private enum CodingKeys: String, CodingKey {
        case idle
        case minCores = "min_cores"
        case name
        case now
    }
}

/// A class or preset with its values.
public struct SizeRow: Codable, Sendable, Equatable {
    /// The fewest cores a lease of it may start with; `nil` while it is automatic.
    public let minCores: Float?

    /// The row's key in the file, such as `cpu_heavy` or a preset's name.
    public let name: String

    /// What a lease of it asks for.
    public let profile: JobProfile

    private enum CodingKeys: String, CodingKey {
        case minCores = "min_cores"
        case name
        case profile
    }
}

/// A built-in row of a size table.
public struct SizeRowSpec: Codable, Sendable, Equatable {
    /// The row's values while the file does not change them.
    public let `default`: JobProfile

    /// The row's minimum cores while the file names none; `nil` follows the field's
    /// automatic rule.
    public let defaultMinCores: Float?

    /// The row as the panel shows it.
    public let label: String

    /// The row's key in the file, such as `cpu_heavy`.
    public let name: String

    private enum CodingKeys: String, CodingKey {
        case `default`
        case defaultMinCores = "default_min_cores"
        case label
        case name
    }
}

/// Where the stop came from.
public enum StoppedBy: String, Codable, Sendable, Equatable {
    /// `allot daemon stop`.
    case cli
    /// Stop service in a panel.
    case panel
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
