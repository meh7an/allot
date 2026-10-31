import AllotContract
import Foundation

/// The sample jobs of the handoff prototype (`JOBS` in `Flyout.dc.html`) as daemon state, so
/// that each `SCEN` scenario can be rebuilt from what allotd would send. The tests check the
/// view models against it, and a Debug build of the app shows it with `--scenario` for
/// comparing against the handoff's state board. The generated types have no public
/// initializers, so states are built as JSON and decoded.
public enum Handoff {
    public static let nowMs: Int64 = 1_792_824_720_000

    public static let idleSession = "e09c7b3a-1111-4000-8000-000000000000"

    /// The scenarios the app can show, in the state board's order; the Resources and first-run
    /// ones come with those views.
    public static let scenarioNames = [
        "empty", "one", "several", "kill-confirm", "killed", "bench-drain", "bench-quiet",
        "bench-run", "bench-done", "bench-noisy", "bench-forced", "down", "max", "icons",
        "c-active", "c-over", "c-queued", "c-bench", "c-confirm", "c-killed", "c-removed",
        "c-refused",
    ]

    /// The scenario whose Kill the scenario stopper refuses, to show the inline error; not on
    /// the handoff's board.
    public static let refusedScenario = "c-refused"

    /// The refusal of ``refusedScenario``, as ``DaemonClient`` words a 409.
    public static let refusedReason = "the bound process 18244 (cargo) is protected"

    // A week before nowMs.
    private static let firstSessionMs = nowMs - 7 * 86_400_000

    private static let jobs: [String: Job] = [
        "cargo": Job(
            id: 1, session: "3f2a9c1e", cwd: "/Users/user/code/allot", cores: 7.8, ramMb: 2970,
            command: "cargo build --release", jobClass: "cpu_heavy", process: "cargo",
            pid: 18244, profile: Profile(cores: 8, ramMb: 4096), elapsed: 186, estimate: 300),
        "py": Job(
            id: 2, session: "b81d44f0", cwd: "/Users/user/code/ml-trainer", cores: 1.9,
            ramMb: 6554, command: "python train.py --epochs 20 --data ./shards",
            jobClass: "gpu_heavy", process: "python3.13", pid: 22108,
            profile: Profile(cores: 2, ramMb: 4096, gpuPct: 80, vramMb: 4096), elapsed: 700,
            estimate: 600),
        "vite": Job(
            id: 3, session: "5d2e8b61",
            cwd: "/Users/user/code/design-tokens-and-a-very-long-folder-name", cores: 3.2,
            ramMb: 1843, command: "pnpm --filter web vite build --mode production",
            jobClass: "mixed", process: "node", pid: 9120,
            profile: Profile(cores: 4, ramMb: 2048, gpuPct: 10, vramMb: 1024), elapsed: 52,
            estimate: 90),
        "go": Job(
            id: 4, session: "9a7c03de", cwd: "/Users/user/code/ingest-svc", cores: 0.9,
            ramMb: 614, command: "go vet ./...", jobClass: "light", process: "go", pid: 14772,
            profile: Profile(cores: 1, ramMb: 1024), elapsed: 20, estimate: 60),
        "npm": Job(
            id: 5, session: "77a0e5d2", cwd: "/Users/user/code/customer-portal-monorepo",
            cores: 0.4, ramMb: 1126, command: "npm run test:e2e", jobClass: "cpu_heavy",
            profile: Profile(cores: 2, ramMb: 4096), waited: 42, waitLeft: 78),
        "npmB": Job(
            id: 6, session: "77a0e5d2", cwd: "/Users/user/code/customer-portal-monorepo",
            cores: 0.4, ramMb: 1126, command: "npm run test:e2e", jobClass: "cpu_heavy",
            profile: Profile(cores: 2, ramMb: 4096), waited: 72),
        "npmN": Job(
            id: 7, session: "77a0e5d2", cwd: "/Users/user/code/customer-portal-monorepo",
            cores: 0.4, ramMb: 1126, command: "npm run test:e2e", jobClass: "cpu_heavy",
            profile: Profile(cores: 2, ramMb: 4096), waited: 196, waitLeft: 17),
        "ctest": Job(
            id: 8, session: "3f2a9c1e", cwd: "/Users/user/code/allot", cores: 7.8, ramMb: 2970,
            command: "cargo test --workspace", profile: Profile(cores: 8, ramMb: 4096),
            waited: 18),
        "bench": Job(
            id: 9, session: "c41f9a07", cwd: "/Users/user/code/parser-perf", cores: 0.2,
            ramMb: 410, command: "cargo bench --bench parser", process: "cargo", pid: 30412,
            profile: Profile(cores: 16, ramMb: 8192), elapsed: 48, estimate: 120, waited: 30),
    ]

    // Executables inside app bundles every Mac has, for the icons scenario; go's is a plain
    // command-line tool, so it keeps its initials.
    private static let exes = [
        "cargo": "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal",
        "py": "/System/Applications/Utilities/Activity Monitor.app/Contents/MacOS/Activity Monitor",
        "vite": "/System/Applications/Calculator.app/Contents/MacOS/Calculator",
        "go": "/opt/homebrew/bin/go",
    ]

    public static func leaseId(_ job: String) -> Int64 {
        jobs[job]?.id ?? 0
    }

    public static func sessionOf(_ job: String) -> String {
        fullSession(jobs[job]?.session ?? "")
    }

    /// The service status and state of a scenario in ``scenarioNames``; the `c-` component
    /// scenarios are their jobs alone, without the idle session.
    ///
    /// - Throws: `CocoaError` for an unknown name, or when the built state does not decode.
    public static func scenario(_ name: String) throws -> (ServiceStatus, UiState) {
        switch name {
        case "empty":
            return try (.running, state(recent: true))
        case "one", "c-active", "c-confirm", "c-killed", refusedScenario:
            return try (.running, state(run: ["cargo"]))
        case "several", "kill-confirm", "killed":
            return try (.running, state(run: ["cargo", "py"], wait: ["npm"], idle: true))
        case "c-removed":
            return try (.running, state(wait: ["npm"]))
        case "bench-drain":
            return try (.running, state(run: ["cargo"], wait: ["npmB"], bench: "drain"))
        case "bench-quiet":
            return try (.running, state(wait: ["npmB"], bench: "quiet"))
        case "bench-run":
            return try (.running, state(wait: ["npmB"], bench: "run"))
        case "bench-done":
            return try (.running, state(wait: ["npmN"], bench: "done"))
        case "bench-noisy":
            return try (.running, state(wait: ["npmB"], bench: "noisy"))
        case "bench-forced":
            return try (.running, state(wait: ["npmB"], bench: "forced"))
        case "down":
            return try (.stopped, state(run: ["cargo"]))
        case "max":
            return try (.running, maxState(exes: false))
        case "icons":
            return try (.running, maxState(exes: true))
        case "c-over":
            return try (.running, state(run: ["py"]))
        case "c-queued":
            return try (.running, state(wait: ["npm", "ctest"]))
        case "c-bench":
            return try (.running, state(bench: "quiet"))
        default:
            throw CocoaError(.featureUnsupported)
        }
    }

    /// The Kill and Remove states a scenario starts in (the prototype's `init`): Kill? with
    /// 3 s left, or Stopped or Removed at the scenario's moment. Empty for the others.
    public static func stops(_ name: String) -> [Int64: StopView] {
        switch name {
        case "kill-confirm", "c-confirm":
            [leaseId("cargo"): StopView(kind: .kill, phase: .confirming, secondsLeft: 3)]
        case "killed", "c-killed":
            [leaseId("cargo"): StopView(kind: .kill, phase: .ended, endedAtMs: nowMs)]
        case "c-removed":
            [leaseId("npm"): StopView(kind: .remove, phase: .ended, endedAtMs: nowMs)]
        default:
            [:]
        }
    }

    /// - Parameters:
    ///   - run: Active jobs, in grant order.
    ///   - wait: Queued jobs, in queue order after a queued benchmark.
    ///   - bench: `drain`, `quiet`, `noisy` (waiting for quiet past the limits), `run`,
    ///     `forced` (running after the longest quiet wait) or `done` (finished 10 s ago);
    ///     `nil` without one.
    ///   - idle: Adds the idle session `e09c7b3a`.
    ///   - recent: Adds the two Recently finished runs of the `empty` scenario.
    ///   - exes: Gives the processes executables inside app bundles, for their icons.
    /// - Throws: When the built state does not decode, which a contract change would cause.
    public static func state(
        run: [String] = [],
        wait: [String] = [],
        bench: String? = nil,
        idle: Bool = false,
        recent: Bool = false,
        exes: Bool = false
    ) throws -> UiState {
        var activeJobs = run.map { active($0, exes: exes) }
        var queuedJobs: [Object] = []
        var used = run + wait
        let running = bench == "run" || bench == "forced"
        if bench != nil {
            used.append("bench")
        }
        if running {
            var benchJob = active("bench", exes: exes, benchmark: true)
            benchJob["quiet_at_start"] = bench == "run"
            activeJobs.append(benchJob)
        } else if let bench, ["drain", "quiet", "noisy"].contains(bench) {
            let (progress, waitLeft) = benchmarkWait(bench, running: run.count)
            queuedJobs.append(queued(
                Self.job("bench"), position: 1, benchmark: true, progress: progress,
                waitLeft: waitLeft))
        }
        for name in wait {
            queuedJobs.append(queued(Self.job(name), position: queuedJobs.count + 1))
        }

        // A benchmark running alone has the machine's load to itself, as on the board.
        var seen: Set<String> = []
        var sessions: [Object] = []
        for job in used.map({ Self.job($0) }) where seen.insert(job.session).inserted {
            sessions.append(running && job.id == leaseId("bench")
                ? session(fullSession(job.session), job.cwd, cores: 15.6, ramMb: 3174)
                : session(fullSession(job.session), job.cwd, cores: job.cores, ramMb: job.ramMb))
        }
        if idle {
            sessions.append(session(idleSession, "/Users/user/code/notes", cores: 0.1, ramMb: 307))
        }

        var recentRuns: [Object] = []
        if bench == "done" {
            recentRuns.append(finishedRun(quietAtStart: true))
        }
        if recent {
            recentRuns = [
                Self.run("cargo", endedAgoSecs: 240, ranSecs: 292),
                Self.run("npm", endedAgoSecs: 660, ranSecs: 130),
            ]
        }
        return try decode(UiState.self, from: [
            "generated_at_ms": nowMs,
            "service": ["version": "0.2.0", "pid": 4242, "uptime_secs": 600],
            "first_session_at_ms": firstSessionMs,
            "sessions": sessions,
            "active": activeJobs,
            "queue": queuedJobs,
            "meters": meters,
            "recent": recentRuns,
            "notices": [],
            "ended": [],
        ])
    }

    /// The benchmark of `bench-done`: it ran 2:04 and ended 10 s before the scenario's moment,
    /// with background noise avg 1.8%, peak 6.2% CPU.
    ///
    /// - Throws: When the built run does not decode.
    public static func finishedBenchmark(quietAtStart: Bool = true) throws -> RecentRun {
        try decode(RecentRun.self, from: finishedRun(quietAtStart: quietAtStart))
    }

    /// `state` without the lease, as allotd lists it once the lease has ended.
    ///
    /// - Throws: When the state does not round-trip, which a contract change would cause.
    public static func without(_ lease: Int64, in state: UiState) throws -> UiState {
        guard var object = try JSONSerialization.jsonObject(
            with: JSONEncoder().encode(state)) as? Object
        else {
            throw CocoaError(.coderReadCorrupt)
        }
        for list in ["active", "queue"] {
            let jobs = object[list] as? [Object] ?? []
            object[list] = jobs.filter { job in
                ((job["lease"] as? Object)?["id"] as? NSNumber)?.int64Value != lease
            }
        }
        return try decode(UiState.self, from: object)
    }

    typealias Object = [String: Any]

    private static var meters: Object {
        [
            "headroom_pct": 15.0,
            "cpu": [
                "logical_cores": 12, "usage_pct": 6.0, "history_pct": [6.0],
                "reserved_cores": 0.0,
            ],
            "ram": ["total_mb": 36864, "used_mb": 14438, "reserved_mb": 0],
            "gpus": [],
            "drives": [],
        ]
    }

    private static func decode<T: Decodable>(_ type: T.Type, from object: Object) throws -> T {
        try JSONDecoder().decode(type, from: JSONSerialization.data(withJSONObject: object))
    }

    private static func job(_ name: String) -> Job {
        // Every name passed here is a key of the table above.
        jobs[name]!
    }

    private static func maxState(exes: Bool) throws -> UiState {
        try state(run: ["cargo", "py", "vite", "go"], wait: ["npm", "ctest"], idle: true,
                  exes: exes)
    }

    private static func fullSession(_ shortId: String) -> String {
        "\(shortId)-1111-4000-8000-000000000000"
    }

    private static func session(
        _ id: String,
        _ cwd: String,
        cores: Double,
        ramMb: UInt64
    ) -> Object {
        [
            "session": id,
            "cwd": cwd,
            "usage": ["cores": cores, "ram_mb": ramMb, "gpu_pct": 0.0, "vram_mb": 0],
        ]
    }

    private static func active(_ name: String, exes: Bool, benchmark: Bool = false) -> Object {
        let job = Self.job(name)
        let exe = exes ? self.exes[name] : nil
        var process: Any = NSNull()
        if let processName = job.process {
            process = [
                "bound": true,
                "exe": exe.map { $0 as Any } ?? NSNull(),
                "name": processName,
                "pid": job.pid,
            ] as Object
        }
        return [
            "lease": lease(job, active: true, benchmark: benchmark),
            "process": process,
            "quiet_at_start": NSNull(),
        ]
    }

    // Draining behind the running jobs, 6 of 10 s of quiet at 3% CPU, or 14% CPU against a 5%
    // limit with 42 s left before allotd grants it anyway.
    private static func benchmarkWait(_ bench: String, running: Int) -> (Object, UInt64?) {
        switch bench {
        case "quiet":
            ([
                "phase": "measuring", "cpu_avg_pct": 3.0, "gpu_avg_pct": 0.0,
                "measured_secs": 6, "needed_secs": 10,
            ], 4)
        case "noisy":
            ([
                "phase": "noisy", "cpu_avg_pct": 14.0, "cpu_limit_pct": 5.0,
                "gpu_avg_pct": 0.0, "gpu_limit_pct": 10.0, "grant_anyway_in_secs": 42,
            ], 42)
        default:
            (["phase": "draining", "active_leases": running], nil)
        }
    }

    private static func queued(
        _ job: Job,
        position: Int,
        benchmark: Bool = false,
        progress: Object? = nil,
        waitLeft: UInt64? = nil
    ) -> Object {
        let left = waitLeft ?? job.waitLeft
        return [
            "lease": lease(job, active: false, benchmark: benchmark),
            "position": position,
            "verdict": [
                "benchmark": progress.map { $0 as Any } ?? NSNull(),
                "fits": false,
                "reasons": ["not enough free cores"],
                "wait_estimate_secs": left.map { $0 as Any } ?? NSNull(),
            ] as Object,
        ]
    }

    private static func lease(_ job: Job, active: Bool, benchmark: Bool) -> Object {
        let grantedAtMs = nowMs - job.elapsed * 1000
        return [
            "id": job.id,
            "holder": [
                "session": fullSession(job.session), "pid": 1000 + job.id,
                "pid_start_time": 1_792_824_000,
            ],
            "kind": benchmark ? "benchmark_exclusive" : "normal",
            "state": active ? "active" : "queued",
            "class": job.jobClass.map { $0 as Any } ?? NSNull(),
            "profile": job.profile.object(estSecs: job.estimate),
            "label": job.command,
            "cwd": job.cwd,
            "requested_at_ms": active ? grantedAtMs - 5000 : nowMs - job.waited * 1000,
            "granted_at_ms": active ? grantedAtMs as Any : NSNull(),
            "expires_at_ms": nowMs + 30_000,
        ]
    }

    private static func finishedRun(quietAtStart: Bool) -> Object {
        let job = Self.job("bench")
        let endedAtMs = nowMs - 10_000
        return [
            "command": job.command,
            "cwd": job.cwd,
            "declared": job.profile.object(estSecs: 300),
            "end_cause": "released",
            "ended_at_ms": endedAtMs,
            "lease_id": job.id,
            "noise": [
                "avg_cpu_pct": 1.8, "avg_gpu_pct": 0.0, "peak_cpu_pct": 6.2,
                "peak_gpu_pct": 0.0, "quiet_at_start": quietAtStart,
            ],
            "session": fullSession(job.session),
            "started_at_ms": endedAtMs - 124_000,
        ]
    }

    private static func run(_ name: String, endedAgoSecs: Int64, ranSecs: Int64) -> Object {
        let job = Self.job(name)
        let endedAtMs = nowMs - endedAgoSecs * 1000
        return [
            "command": job.command,
            "cwd": job.cwd,
            "declared": job.profile.object(estSecs: 300),
            "end_cause": "released",
            "ended_at_ms": endedAtMs,
            "lease_id": job.id + 100,
            "session": fullSession(job.session),
            "started_at_ms": endedAtMs - ranSecs * 1000,
        ]
    }

    private struct Profile {
        var cores: Double
        var ramMb: UInt64
        var gpuPct: Double = 0
        var vramMb: UInt64 = 0

        func object(estSecs: Int64) -> Object {
            [
                "cores": cores, "ram_mb": ramMb, "gpu_pct": gpuPct, "vram_mb": vramMb,
                "est_secs": estSecs,
            ]
        }
    }

    // elapsed and estimate are an active job's seconds since its grant and its estimate;
    // waited and waitLeft a queued job's seconds since its request and allotd's remaining wait.
    private struct Job {
        var id: Int64
        var session: String
        var cwd: String
        var cores: Double
        var ramMb: UInt64
        var command: String
        var jobClass: String?
        var process: String?
        var pid: UInt32 = 0
        var profile: Profile
        var elapsed: Int64 = 0
        var estimate: Int64 = 300
        var waited: Int64 = 0
        var waitLeft: UInt64?
    }
}
