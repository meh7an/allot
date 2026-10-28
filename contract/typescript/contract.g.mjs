// Generated from the Rust types behind contract/*.schema.json by
// shared/src/contract/typescript.rs. Do not edit; rerun
// `ALLOT_UPDATE_CONTRACT=1 cargo test -p allot-shared contract` instead.
//
// ES2016 with JSDoc types, so GJS and QML import it as is and tsc checks it. Decode parsed
// JSON with the decode functions at the end: they check every field, fill the defaults allotd
// leaves out, and drop members they do not know, as serde ignores them.

/**
 * Response of `GET /ui/state`: everything the flyout shows, in one poll.
 *
 * @typedef {object} UiState
 * @property {ActiveJob[]} active
 *   Active leases, oldest grant first.
 * @property {EndedLease[]} ended
 *   Leases that ended in the last minute, however they ended, newest first, so the flyout
 *   can hold a card that just went away with how it ended. Older allotd builds send none.
 * @property {number | null} first_session_at_ms
 *   When a Claude session first registered with this install, or `null` when none ever
 *   has, which shows the first-run card.
 * @property {number} generated_at_ms
 *   When the state was built, in milliseconds since the Unix epoch. Elapsed and waited
 *   times are measured against it, so they do not depend on the tray app's clock.
 * @property {Meters} meters
 *   Machine meters for the Resources card.
 * @property {Notice[]} notices
 *   Kills and removals, with whether Claude has been told yet.
 * @property {QueuedJob[]} queue
 *   Queued leases, in queue order.
 * @property {RecentRun[]} recent
 *   Recently finished jobs, newest first.
 * @property {HealthResponse} service
 *   The daemon itself.
 * @property {UiSession[]} sessions
 *   Every registered session, with or without leases.
 */

/**
 * Response of `POST /leases/{id}/kill`.
 *
 * @typedef {object} KillResponse
 * @property {UnkilledProcess[]} failed
 *   Processes that could not be terminated, with what the OS said.
 * @property {Lease} lease
 *   The lease, ended with cause `killed`.
 * @property {UnkilledProcess[]} spared
 *   Processes in the tree left running on purpose, each with everything below it.
 * @property {KilledProcess[]} terminated
 *   Processes terminated, root first.
 * @property {string | null} untouched_reason
 *   Why no process was terminated: the lease had no bound process tree, or its command had
 *   already ended. The lease ended anyway. `null` when the bound tree was killed.
 */

/**
 * Response of `POST /leases/{id}/remove`.
 *
 * @typedef {object} RemoveResponse
 * @property {Lease} lease
 *   The lease, ended with cause `removed` before it started.
 */

/**
 * How well a GPU source measures something per process.
 *
 * - `exact`: Measured per process.
 * - `estimated`: Measured, but only roughly; the machine total is the reliable figure.
 * - `unavailable`: Not measured: per-process values read 0.
 *
 * @typedef {"exact" | "estimated" | "unavailable"} Accuracy
 */

/**
 * An active lease and the process its command runs as.
 *
 * @typedef {object} ActiveJob
 * @property {Lease} lease
 *   The lease.
 * @property {JobProcess | null} process
 *   The command's process, or `null` when the session has no process to show.
 * @property {boolean | null} quiet_at_start
 *   For a benchmark lease, whether the machine was quiet when it was granted; `false` means
 *   it was granted anyway after the longest quiet wait. `null` for other leases and when
 *   the daemon restarted since the grant.
 */

/**
 * Where a waiting benchmark lease stands, so its holder can report progress.
 *
 * @typedef {(
 *   | BenchmarkProgressDraining
 *   | BenchmarkProgressMeasuring
 *   | BenchmarkProgressNoisy
 * )} BenchmarkProgress
 */

/**
 * Other leases are still active; nothing new starts until they end.
 *
 * @typedef {object} BenchmarkProgressDraining
 * @property {"draining"} phase
 * @property {number} active_leases
 *   Active leases left.
 */

/**
 * The machine has drained and its background load is being averaged.
 *
 * @typedef {object} BenchmarkProgressMeasuring
 * @property {"measuring"} phase
 * @property {number} cpu_avg_pct
 *   Average total CPU load so far, in percent.
 * @property {number} gpu_avg_pct
 *   Average load of the busiest GPU engine so far, in percent.
 * @property {number} measured_secs
 *   Seconds averaged so far.
 * @property {number} needed_secs
 *   Seconds the average must cover.
 */

/**
 * The average is above the quiet limits. The lease is granted anyway, marked as not
 * quiet, once `grant_anyway_in_secs` have passed.
 *
 * @typedef {object} BenchmarkProgressNoisy
 * @property {"noisy"} phase
 * @property {number} cpu_avg_pct
 *   Average total CPU load over the quiet window, in percent.
 * @property {number} cpu_limit_pct
 *   CPU limit of a quiet machine.
 * @property {number} gpu_avg_pct
 *   Average load of the busiest GPU engine over the quiet window, in percent.
 * @property {number} gpu_limit_pct
 *   GPU limit of a quiet machine.
 * @property {number} grant_anyway_in_secs
 *   Seconds until the lease is granted without quiet.
 */

/**
 * One process, identified across PID reuse.
 *
 * @typedef {object} BoundProcess
 * @property {number} pid
 *   Process ID.
 * @property {number} start_time
 *   Start time of `pid` in seconds since the Unix epoch, as in `Holder::pid_start_time`.
 */

/**
 * Processor load.
 *
 * @typedef {object} CpuMeter
 * @property {number[]} history_pct
 *   Recent once-a-second loads for the sparkline, oldest first, at most 24.
 * @property {number} logical_cores
 *   Number of logical processors.
 * @property {number} reserved_cores
 *   Logical cores reserved by active leases.
 * @property {number} usage_pct
 *   Total load over the last second, as a percentage of all logical processors.
 */

/**
 * Space and activity of one fixed drive.
 *
 * @typedef {object} DriveUsage
 * @property {string} name
 *   Drive letter with its colon on Windows, for example `C:`; mount point elsewhere, for
 *   example `/home`.
 * @property {number} read_bytes_per_sec
 *   Bytes read per second over the last second.
 * @property {number} total_mb
 *   Capacity of the volume.
 * @property {number} used_mb
 *   Space in use.
 * @property {number} write_bytes_per_sec
 *   Bytes written per second over the last second.
 */

/**
 * Why a lease ended.
 *
 * Kept apart from `LeaseState`, whose values are fixed by the database's CHECK constraints
 * and by older `allot-mcp` builds that reject unknown states.
 *
 * - `released`: Its holder released it.
 * - `expired`: The daemon ended it because the heartbeat stopped or the holder died.
 * - `killed`: The user stopped the job and its process tree.
 * - `removed`: The user removed the request from the queue before it started.
 *
 * @typedef {"released" | "expired" | "killed" | "removed"} EndCause
 */

/**
 * A lease that ended a moment ago: released by its holder, ended by allotd, killed or removed.
 *
 * @typedef {object} EndedLease
 * @property {EndCause} cause
 *   How it ended.
 * @property {string} command
 *   The lease's label, usually the command.
 * @property {number} ended_at_ms
 *   When it ended, in milliseconds since the Unix epoch.
 * @property {number | null} granted_at_ms
 *   When it was granted, or `null` for a request that never started.
 * @property {number} lease_id
 *   The lease.
 * @property {string} reason
 *   allotd's reason for the end, for example "no command running and the session idle for
 *   180s"; empty when none was recorded.
 * @property {string} session
 *   Session that held it.
 */

/**
 * Caps the OS enforces on a lease's cgroup; each is `null` when not enforced.
 *
 * @typedef {object} EnforcedLimits
 * @property {number | null} cpu_cores
 *   CPU time limit in logical cores.
 * @property {number | null} memory_high_mb
 *   Memory above which the OS throttles the command and reclaims its memory.
 * @property {number | null} memory_max_mb
 *   Memory at which the OS kills the command.
 */

/**
 * One GPU adapter.
 *
 * @typedef {object} GpuMeter
 * @property {number} busiest_pct
 *   Load of the busiest engine group over the last second, in percent.
 * @property {string} name
 *   Adapter name, for example `NVIDIA GeForce RTX 4070`.
 * @property {Accuracy} process_load
 *   How well each job's GPU load is measured on this adapter.
 * @property {number} reserved_pct
 *   Engine load reserved by active leases, in percent. Leases reserve on the adapter with
 *   the most video memory only, so it is 0 on every other adapter.
 * @property {number} reserved_vram_mb
 *   Video memory reserved by active leases; 0 on every adapter but the reserving one. On a
 *   unified-memory adapter it comes out of RAM, on top of the RAM meter's reservation.
 * @property {boolean} unified_memory
 *   Whether the adapter's video memory is the machine's RAM, as on Apple Silicon.
 * @property {number | null} vram_total_mb
 *   Dedicated video memory installed, when the OS reports it; `null` on a unified-memory
 *   adapter.
 * @property {number} vram_used_mb
 *   Dedicated video memory in use; on a unified-memory adapter, the RAM the GPU has
 *   allocated.
 */

/**
 * Response of `GET /health`.
 *
 * @typedef {object} HealthResponse
 * @property {string | null} config_error
 *   Why the last change to `config.toml` was rejected; the daemon keeps the config it had
 *   until the file is fixed.
 * @property {number} pid
 *   Daemon process ID.
 * @property {number} uptime_secs
 *   Seconds since the daemon started.
 * @property {string} version
 *   Daemon version, from its `Cargo.toml`.
 */

/**
 * The process a session's leases live and die with.
 *
 * @typedef {object} Holder
 * @property {number} pid
 *   PID of the session's Claude Code process (`claude.exe` on Windows, `claude` elsewhere),
 *   the direct parent of its MCP server and its hooks.
 * @property {number} pid_start_time
 *   Start time of `pid` in seconds since the Unix epoch. Every OS reuses PIDs, so a process
 *   with the same PID but another start time is not the holder.
 * @property {string} session
 *   Session the leases belong to.
 */

/**
 * How a lease's processes are found and stopped.
 *
 * - `cgroup`: A cgroup of its own: exact usage, and a kill ends every process in it at once.
 * - `tree`: The process tree under the command's root, followed sample by sample.
 *
 * @typedef {"cgroup" | "tree"} IsolationKind
 */

/**
 * Named job profiles. Their values come from the `[classes]` section of the config.
 *
 * - `light`: Short jobs that barely load the machine.
 * - `cpu_heavy`: Compiles, test suites, and other CPU-bound work.
 * - `gpu_heavy`: Training, rendering, games, and other GPU-bound work.
 * - `mixed`: Jobs that load both the CPU and the GPU.
 *
 * @typedef {"light" | "cpu_heavy" | "gpu_heavy" | "mixed"} JobClass
 */

/**
 * The process shown for a job.
 *
 * @typedef {object} JobProcess
 * @property {boolean} bound
 *   Whether it belongs to the command's bound process tree, the only tree a kill ends: its
 *   busiest process, or its root while the tree is idle. `false` means it is the session's
 *   busiest process, shown for display only.
 * @property {string | null} exe
 *   Full path of the executable, for its icon; `null` when it could not be read.
 * @property {string} name
 *   Executable name, for example `cargo.exe`.
 * @property {number} pid
 *   Process ID.
 */

/**
 * Resources a job is expected to need while it runs.
 *
 * @typedef {object} JobProfile
 * @property {number} cores
 *   Logical cores the job keeps busy.
 * @property {number} est_secs
 *   Expected run time.
 * @property {number} gpu_pct
 *   GPU engine load as a percentage of one adapter.
 * @property {number} ram_mb
 *   Peak resident memory.
 * @property {number} vram_mb
 *   Peak dedicated video memory.
 */

/**
 * A process a kill terminated.
 *
 * @typedef {object} KilledProcess
 * @property {string} name
 *   Executable name, for example `cargo.exe`.
 * @property {number} pid
 *   Process ID.
 */

/**
 * A reservation of machine resources for one job.
 *
 * @typedef {object} Lease
 * @property {LeaseBinding | null} binding
 *   The command the lease's holder last reported running under it; `null` until the hook
 *   reports one.
 * @property {JobClass | null} class
 *   Class the profile was taken from, or `null` for a custom profile.
 * @property {string} cwd
 *   Working directory of the command, or empty when the requester did not say.
 * @property {EndCause | null} end_cause
 *   Why the lease ended; `null` while it is queued or active, and for leases that ended
 *   before causes were recorded.
 * @property {number | null} ended_at_ms
 *   When the lease was released or expired.
 * @property {number} expires_at_ms
 *   When the lease expires unless a heartbeat arrives first.
 * @property {number | null} granted_at_ms
 *   When the lease became active.
 * @property {Holder} holder
 *   Session that owns the lease.
 * @property {number} id
 *   Lease ID.
 * @property {LeaseIsolation | null} isolation
 *   How the lease's command is kept apart from the rest of the machine; `null` where allotd
 *   does not report it, as on Windows.
 * @property {LeaseKind} kind
 *   How the lease shares the machine.
 * @property {string} label
 *   What the lease is for, usually the command that will run.
 * @property {JobProfile} profile
 *   Resources reserved while the lease is active.
 * @property {number} requested_at_ms
 *   When the lease was requested, in milliseconds since the Unix epoch.
 * @property {LeaseState} state
 *   Lifecycle state.
 */

/**
 * The command a lease covers, as the PreToolUse hook reported it, and the process tree it
 * runs as once that is found. A later report for the same lease replaces it.
 *
 * @typedef {object} LeaseBinding
 * @property {number} bound_at_ms
 *   When the hook reported the command, in milliseconds since the Unix epoch. Processes
 *   that started before it are never part of the command.
 * @property {BoundProcess | null} root
 *   Root of the command's process tree; `null` until it is found.
 * @property {string} tool_use_id
 *   Claude Code's ID for the tool call that runs the command.
 */

/**
 * How a lease's command is accounted for and stopped.
 *
 * @typedef {object} LeaseIsolation
 * @property {IsolationKind} kind
 *   Whether the command runs in its own cgroup or is followed as a process tree.
 * @property {EnforcedLimits} limits
 *   Caps the OS enforces on the command.
 * @property {string | null} scope
 *   Name of the systemd scope that holds the command, for example
 *   `allot-lease-42.scope`; `null` without a cgroup.
 */

/**
 * How a lease shares the machine.
 *
 * - `normal`: Runs alongside other leases as long as their reservations fit.
 * - `benchmark_exclusive`: Runs alone on a quiet machine so that measurements are not disturbed.
 *
 * @typedef {"normal" | "benchmark_exclusive"} LeaseKind
 */

/**
 * Where a lease is in its lifecycle.
 *
 * - `queued`: Waiting for resources.
 * - `active`: Granted; its reservation counts against the machine.
 * - `released`: Released by its holder.
 * - `expired`: Ended by the daemon because the heartbeat stopped or the holder died.
 *
 * @typedef {"queued" | "active" | "released" | "expired"} LeaseState
 */

/**
 * Machine meters with the share that leases reserve.
 *
 * @typedef {object} Meters
 * @property {CpuMeter} cpu
 *   Processor load.
 * @property {DriveUsage[]} drives
 *   Every fixed drive, by drive letter on Windows and by mount point elsewhere.
 * @property {GpuMeter[]} gpus
 *   Every GPU adapter.
 * @property {number} headroom_pct
 *   Percentage of each resource that leases never take, from `admission.headroom_pct`.
 * @property {RamMeter} ram
 *   Physical memory.
 */

/**
 * Load from processes outside the holder's session while a benchmark lease was active.
 *
 * CPU is the summed per-process load of every other session and of processes outside all
 * sessions. Kernel and interrupt time belongs to no process, so it is not counted: it would
 * otherwise charge the benchmark's own system time as noise.
 *
 * @typedef {object} NoiseReport
 * @property {number} avg_cpu_pct
 *   Average CPU load of other processes, as a percentage of all logical processors.
 * @property {number} avg_gpu_pct
 *   Average GPU engine load of other processes, as a percentage of one adapter.
 * @property {number} peak_cpu_pct
 *   Peak CPU load of other processes.
 * @property {number} peak_gpu_pct
 *   Peak GPU engine load of other processes.
 * @property {boolean} quiet_at_start
 *   Whether the machine met the quiet limits when the lease was granted. `false` means it
 *   was granted anyway after the longest quiet wait.
 */

/**
 * A kill or removal that Claude must hear about once.
 *
 * @typedef {object} Notice
 * @property {EndCause} cause
 *   What the user did: `EndCause::Killed` or `EndCause::Removed`.
 * @property {string} command
 *   The lease's label, usually the command.
 * @property {number | null} delivered_at_ms
 *   When a hook or tool result told the session, or `null` while it has not been told.
 * @property {number} ended_at_ms
 *   When the lease ended, in milliseconds since the Unix epoch.
 * @property {number} lease_id
 *   Lease the notice is about. A lease ends once, so this also identifies the notice.
 * @property {string} session
 *   Session to tell.
 * @property {string | null} tool_use_id
 *   Claude Code's ID for the tool call the lease was bound to, or `null` when no hook
 *   reported one. A hook compares it with its own call to say "this command".
 */

/**
 * A queued lease and why it waits.
 *
 * @typedef {object} QueuedJob
 * @property {Lease} lease
 *   The lease.
 * @property {number} position
 *   Place in the queue, starting at 1.
 * @property {Verdict} verdict
 *   Why it waits, the expected wait, and benchmark progress when it is a benchmark.
 */

/**
 * Physical memory.
 *
 * @typedef {object} RamMeter
 * @property {number} reserved_mb
 *   Memory reserved by active leases.
 * @property {number} total_mb
 *   Installed physical memory.
 * @property {number} used_mb
 *   Physical memory in use.
 */

/**
 * A finished job.
 *
 * @typedef {object} RecentRun
 * @property {string} command
 *   The lease's label, usually the command as it was run.
 * @property {string} cwd
 *   Working directory of the command.
 * @property {JobProfile} declared
 *   Profile the lease reserved.
 * @property {EndCause | null} end_cause
 *   Why it ended; `null` for runs recorded before causes were.
 * @property {number} ended_at_ms
 *   End of the run, in milliseconds since the Unix epoch.
 * @property {number | null} lease_id
 *   Lease the job ran under, if the lease is still stored.
 * @property {NoiseReport | null} noise
 *   Background load during the run; present only for benchmark leases.
 * @property {string | null} session
 *   Session that ran it; `null` for runs recorded before sessions were.
 * @property {number} started_at_ms
 *   Start of the run, in milliseconds since the Unix epoch.
 */

/**
 * Resources used by a process or a group of processes, in the units of `JobProfile`.
 *
 * @typedef {object} ResourceUsage
 * @property {number} cores
 *   CPU load in logical cores, for example `2.5` for two and a half busy cores.
 * @property {number} gpu_pct
 *   GPU engine load as a percentage of one adapter.
 * @property {number} ram_mb
 *   Resident memory; on macOS the physical footprint, as Activity Monitor shows it.
 * @property {number} vram_mb
 *   Dedicated video memory.
 */

/**
 * One registered Claude session.
 *
 * @typedef {object} UiSession
 * @property {string | null} cwd
 *   Working directory of the session's most recent lease, which names its project; `null`
 *   when no lease gave one.
 * @property {string} session
 *   Session ID.
 * @property {ResourceUsage} usage
 *   Live usage of the session's whole process subtree.
 */

/**
 * A process in a killed tree that is still running.
 *
 * @typedef {object} UnkilledProcess
 * @property {string} name
 *   Executable name, for example `claude.exe`.
 * @property {number} pid
 *   Process ID.
 * @property {string} reason
 *   Why it is still running.
 */

/**
 * The admission engine's answer. Response of `POST /can-fit`.
 *
 * @typedef {object} Verdict
 * @property {BenchmarkProgress | null} benchmark
 *   Where a benchmark at the head of the queue stands; `null` for other requests.
 * @property {boolean} fits
 *   Whether the job could start now.
 * @property {string[]} reasons
 *   Human-readable explanations, one per limiting or notable factor.
 * @property {number | null} wait_estimate_secs
 *   Expected wait before the job fits, when the daemon can estimate it.
 */

/**
 * How a value is checked: a primitive or definition `type`, or an `array` of items, with
 * whether it may be `null` and, for an optional field, the value a missing field takes.
 *
 * @typedef {({ type: string } | { array: ValueSpec }) & FieldOptions} ValueSpec
 */

/**
 * @typedef {object} FieldOptions
 * @property {boolean} [nullable]
 * @property {null | boolean | string | never[]} [absent]
 */

/** @typedef {{ fields: Readonly<Record<string, ValueSpec>> }} RecordSpec */

/**
 * @typedef {(
 *   | RecordSpec
 *   | { values: readonly string[] }
 *   | { tag: string, variants: Readonly<Record<string, RecordSpec>> }
 * )} DefinitionSpec
 */

/** @type {Readonly<Record<string, DefinitionSpec>>} */
const TYPES = {
  UiState: {
    fields: {
      active: { array: { type: "ActiveJob" } },
      ended: { array: { type: "EndedLease" }, absent: [] },
      first_session_at_ms: { type: "int64", nullable: true, absent: null },
      generated_at_ms: { type: "int64" },
      meters: { type: "Meters" },
      notices: { array: { type: "Notice" } },
      queue: { array: { type: "QueuedJob" } },
      recent: { array: { type: "RecentRun" } },
      service: { type: "HealthResponse" },
      sessions: { array: { type: "UiSession" } },
    },
  },
  KillResponse: {
    fields: {
      failed: { array: { type: "UnkilledProcess" } },
      lease: { type: "Lease" },
      spared: { array: { type: "UnkilledProcess" } },
      terminated: { array: { type: "KilledProcess" } },
      untouched_reason: { type: "string", nullable: true, absent: null },
    },
  },
  RemoveResponse: {
    fields: {
      lease: { type: "Lease" },
    },
  },
  Accuracy: { values: ["exact", "estimated", "unavailable"] },
  ActiveJob: {
    fields: {
      lease: { type: "Lease" },
      process: { type: "JobProcess", nullable: true, absent: null },
      quiet_at_start: { type: "boolean", nullable: true, absent: null },
    },
  },
  BenchmarkProgress: {
    tag: "phase",
    variants: {
      draining: {
        fields: {
          active_leases: { type: "uint32" },
        },
      },
      measuring: {
        fields: {
          cpu_avg_pct: { type: "float" },
          gpu_avg_pct: { type: "float" },
          measured_secs: { type: "uint64" },
          needed_secs: { type: "uint64" },
        },
      },
      noisy: {
        fields: {
          cpu_avg_pct: { type: "float" },
          cpu_limit_pct: { type: "float" },
          gpu_avg_pct: { type: "float" },
          gpu_limit_pct: { type: "float" },
          grant_anyway_in_secs: { type: "uint64" },
        },
      },
    },
  },
  BoundProcess: {
    fields: {
      pid: { type: "uint32" },
      start_time: { type: "uint64" },
    },
  },
  CpuMeter: {
    fields: {
      history_pct: { array: { type: "float" } },
      logical_cores: { type: "uint32" },
      reserved_cores: { type: "float" },
      usage_pct: { type: "float" },
    },
  },
  DriveUsage: {
    fields: {
      name: { type: "string" },
      read_bytes_per_sec: { type: "uint64" },
      total_mb: { type: "uint64" },
      used_mb: { type: "uint64" },
      write_bytes_per_sec: { type: "uint64" },
    },
  },
  EndCause: { values: ["released", "expired", "killed", "removed"] },
  EndedLease: {
    fields: {
      cause: { type: "EndCause" },
      command: { type: "string" },
      ended_at_ms: { type: "int64" },
      granted_at_ms: { type: "int64", nullable: true, absent: null },
      lease_id: { type: "int64" },
      reason: { type: "string" },
      session: { type: "string" },
    },
  },
  EnforcedLimits: {
    fields: {
      cpu_cores: { type: "float", nullable: true, absent: null },
      memory_high_mb: { type: "uint64", nullable: true, absent: null },
      memory_max_mb: { type: "uint64", nullable: true, absent: null },
    },
  },
  GpuMeter: {
    fields: {
      busiest_pct: { type: "float" },
      name: { type: "string" },
      process_load: { type: "Accuracy", absent: "exact" },
      reserved_pct: { type: "float" },
      reserved_vram_mb: { type: "uint64" },
      unified_memory: { type: "boolean", absent: false },
      vram_total_mb: { type: "uint64", nullable: true, absent: null },
      vram_used_mb: { type: "uint64" },
    },
  },
  HealthResponse: {
    fields: {
      config_error: { type: "string", nullable: true, absent: null },
      pid: { type: "uint32" },
      uptime_secs: { type: "uint64" },
      version: { type: "string" },
    },
  },
  Holder: {
    fields: {
      pid: { type: "uint32" },
      pid_start_time: { type: "uint64" },
      session: { type: "string" },
    },
  },
  IsolationKind: { values: ["cgroup", "tree"] },
  JobClass: { values: ["light", "cpu_heavy", "gpu_heavy", "mixed"] },
  JobProcess: {
    fields: {
      bound: { type: "boolean" },
      exe: { type: "string", nullable: true, absent: null },
      name: { type: "string" },
      pid: { type: "uint32" },
    },
  },
  JobProfile: {
    fields: {
      cores: { type: "float" },
      est_secs: { type: "uint64" },
      gpu_pct: { type: "float" },
      ram_mb: { type: "uint64" },
      vram_mb: { type: "uint64" },
    },
  },
  KilledProcess: {
    fields: {
      name: { type: "string" },
      pid: { type: "uint32" },
    },
  },
  Lease: {
    fields: {
      binding: { type: "LeaseBinding", nullable: true, absent: null },
      class: { type: "JobClass", nullable: true, absent: null },
      cwd: { type: "string", absent: "" },
      end_cause: { type: "EndCause", nullable: true, absent: null },
      ended_at_ms: { type: "int64", nullable: true, absent: null },
      expires_at_ms: { type: "int64" },
      granted_at_ms: { type: "int64", nullable: true, absent: null },
      holder: { type: "Holder" },
      id: { type: "int64" },
      isolation: { type: "LeaseIsolation", nullable: true, absent: null },
      kind: { type: "LeaseKind" },
      label: { type: "string" },
      profile: { type: "JobProfile" },
      requested_at_ms: { type: "int64" },
      state: { type: "LeaseState" },
    },
  },
  LeaseBinding: {
    fields: {
      bound_at_ms: { type: "int64" },
      root: { type: "BoundProcess", nullable: true, absent: null },
      tool_use_id: { type: "string" },
    },
  },
  LeaseIsolation: {
    fields: {
      kind: { type: "IsolationKind" },
      limits: { type: "EnforcedLimits" },
      scope: { type: "string", nullable: true, absent: null },
    },
  },
  LeaseKind: { values: ["normal", "benchmark_exclusive"] },
  LeaseState: { values: ["queued", "active", "released", "expired"] },
  Meters: {
    fields: {
      cpu: { type: "CpuMeter" },
      drives: { array: { type: "DriveUsage" } },
      gpus: { array: { type: "GpuMeter" } },
      headroom_pct: { type: "float" },
      ram: { type: "RamMeter" },
    },
  },
  NoiseReport: {
    fields: {
      avg_cpu_pct: { type: "float" },
      avg_gpu_pct: { type: "float" },
      peak_cpu_pct: { type: "float" },
      peak_gpu_pct: { type: "float" },
      quiet_at_start: { type: "boolean" },
    },
  },
  Notice: {
    fields: {
      cause: { type: "EndCause" },
      command: { type: "string" },
      delivered_at_ms: { type: "int64", nullable: true, absent: null },
      ended_at_ms: { type: "int64" },
      lease_id: { type: "int64" },
      session: { type: "string" },
      tool_use_id: { type: "string", nullable: true, absent: null },
    },
  },
  QueuedJob: {
    fields: {
      lease: { type: "Lease" },
      position: { type: "uint32" },
      verdict: { type: "Verdict" },
    },
  },
  RamMeter: {
    fields: {
      reserved_mb: { type: "uint64" },
      total_mb: { type: "uint64" },
      used_mb: { type: "uint64" },
    },
  },
  RecentRun: {
    fields: {
      command: { type: "string" },
      cwd: { type: "string" },
      declared: { type: "JobProfile" },
      end_cause: { type: "EndCause", nullable: true, absent: null },
      ended_at_ms: { type: "int64" },
      lease_id: { type: "int64", nullable: true, absent: null },
      noise: { type: "NoiseReport", nullable: true, absent: null },
      session: { type: "string", nullable: true, absent: null },
      started_at_ms: { type: "int64" },
    },
  },
  ResourceUsage: {
    fields: {
      cores: { type: "float" },
      gpu_pct: { type: "float" },
      ram_mb: { type: "uint64" },
      vram_mb: { type: "uint64" },
    },
  },
  UiSession: {
    fields: {
      cwd: { type: "string", nullable: true, absent: null },
      session: { type: "string" },
      usage: { type: "ResourceUsage" },
    },
  },
  UnkilledProcess: {
    fields: {
      name: { type: "string" },
      pid: { type: "uint32" },
      reason: { type: "string" },
    },
  },
  Verdict: {
    fields: {
      benchmark: { type: "BenchmarkProgress", nullable: true, absent: null },
      fits: { type: "boolean" },
      reasons: { array: { type: "string" } },
      wait_estimate_secs: { type: "uint64", nullable: true, absent: null },
    },
  },
};

/** Thrown when a value does not match the contract; the message starts with its path. */
export class ContractError extends Error {
  /**
   * @param {string} path
   * @param {string} problem
   */
  constructor(path, problem) {
    super(`${path}: ${problem}`);
    this.name = "ContractError";
    this.path = path;
  }
}

/** @type {Readonly<Record<"int64" | "uint64" | "uint32", readonly [number, number]>>} */
const INTEGER_RANGES = {
  int64: [-Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER],
  uint64: [0, Number.MAX_SAFE_INTEGER],
  uint32: [0, 4294967295],
};

/**
 * @param {object} object
 * @param {string} key
 * @returns {boolean}
 */
function hasOwn(object, key) {
  return Object.prototype.hasOwnProperty.call(object, key);
}

/**
 * The value under `key`, unless `key` is missing or only inherited, such as `constructor`.
 *
 * @template T
 * @param {Readonly<Record<string, T>>} record
 * @param {string} key
 * @returns {T | undefined}
 */
function own(record, key) {
  return hasOwn(record, key) ? record[key] : undefined;
}

/**
 * @param {unknown} value
 * @param {ValueSpec} spec
 * @param {string} path
 * @returns {unknown}
 */
function decodeValue(value, spec, path) {
  if (value === null && spec.nullable) {
    return null;
  }
  if ("array" in spec) {
    if (!Array.isArray(value)) {
      throw new ContractError(path, "expected an array");
    }
    const item = spec.array;
    return value.map((element, index) => decodeValue(element, item, `${path}[${index}]`));
  }
  return decodeType(value, spec.type, path);
}

/**
 * @param {unknown} value
 * @param {string} type
 * @param {string} path
 * @returns {unknown}
 */
function decodeType(value, type, path) {
  switch (type) {
    case "boolean":
    case "string":
      if (typeof value !== type) {
        throw new ContractError(path, `expected a ${type}`);
      }
      return value;
    case "int64":
    case "uint64":
    case "uint32": {
      const [minimum, maximum] = INTEGER_RANGES[type];
      if (
        typeof value !== "number" ||
        !Number.isInteger(value) ||
        value < minimum ||
        value > maximum
      ) {
        throw new ContractError(path, `expected an integer from ${minimum} to ${maximum}`);
      }
      return value;
    }
    case "float":
    case "double":
      if (typeof value !== "number" || !isFinite(value)) {
        throw new ContractError(path, "expected a number");
      }
      return value;
    default: {
      const definition = own(TYPES, type);
      if (definition === undefined) {
        throw new ContractError(path, `no type ${type} in the contract`);
      }
      return decodeDefinition(value, definition, path);
    }
  }
}

/**
 * @param {unknown} value
 * @param {DefinitionSpec} definition
 * @param {string} path
 * @returns {unknown}
 */
function decodeDefinition(value, definition, path) {
  if ("values" in definition) {
    if (typeof value !== "string" || definition.values.indexOf(value) < 0) {
      throw new ContractError(path, `unknown value ${JSON.stringify(value)}`);
    }
    return value;
  }
  const object = decodeObject(value, path);
  if ("tag" in definition) {
    const tag = object[definition.tag];
    const variant = typeof tag === "string" ? own(definition.variants, tag) : undefined;
    if (variant === undefined) {
      throw new ContractError(
        `${path}.${definition.tag}`,
        `unknown variant ${JSON.stringify(tag)}`,
      );
    }
    /** @type {Record<string, unknown>} */
    const decoded = {};
    decoded[definition.tag] = tag;
    return Object.assign(decoded, decodeFields(object, variant, path));
  }
  return decodeFields(object, definition, path);
}

/**
 * @param {unknown} value
 * @param {string} path
 * @returns {Record<string, unknown>}
 */
function decodeObject(value, path) {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new ContractError(path, "expected an object");
  }
  return /** @type {Record<string, unknown>} */ (value);
}

/**
 * @param {Record<string, unknown>} object
 * @param {RecordSpec} record
 * @param {string} path
 * @returns {Record<string, unknown>}
 */
function decodeFields(object, record, path) {
  /** @type {Record<string, unknown>} */
  const decoded = {};
  for (const name of Object.keys(record.fields)) {
    // Object.keys lists only names the record has.
    const spec = /** @type {ValueSpec} */ (record.fields[name]);
    const fieldPath = `${path}.${name}`;
    if (hasOwn(object, name)) {
      decoded[name] = decodeValue(object[name], spec, fieldPath);
    } else if ("absent" in spec) {
      decoded[name] = Array.isArray(spec.absent) ? [] : spec.absent;
    } else {
      throw new ContractError(fieldPath, "missing");
    }
  }
  return decoded;
}

/**
 * Decodes a parsed `UiState` body.
 *
 * @param {unknown} json The value `JSON.parse` returned.
 * @returns {UiState}
 * @throws {ContractError} When the value does not match the contract.
 */
export function decodeUiState(json) {
  return /** @type {UiState} */ (decodeType(json, "UiState", "UiState"));
}

/**
 * Decodes a parsed `KillResponse` body.
 *
 * @param {unknown} json The value `JSON.parse` returned.
 * @returns {KillResponse}
 * @throws {ContractError} When the value does not match the contract.
 */
export function decodeKillResponse(json) {
  return /** @type {KillResponse} */ (decodeType(json, "KillResponse", "KillResponse"));
}

/**
 * Decodes a parsed `RemoveResponse` body.
 *
 * @param {unknown} json The value `JSON.parse` returned.
 * @returns {RemoveResponse}
 * @throws {ContractError} When the value does not match the contract.
 */
export function decodeRemoveResponse(json) {
  return /** @type {RemoveResponse} */ (decodeType(json, "RemoveResponse", "RemoveResponse"));
}
