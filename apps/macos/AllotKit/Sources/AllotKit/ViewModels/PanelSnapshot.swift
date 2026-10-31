import AllotContract

public enum HeaderGlyph: Sendable, Equatable {
    /// The dimmed mark.
    case idle

    /// The solid mark: a job runs, or a benchmark runs alone.
    case busy

    /// Red error icon: the service is not serving state.
    case error
}

public enum JobGroupKind: Sendable, Equatable {
    case benchmark
    case running
    case waiting
}

/// Where the live benchmark stands, as the summary words it.
public enum BenchmarkPhase: Sendable, Equatable {
    /// Queued while other leases finish, or queued behind other requests.
    case draining

    /// The machine has drained and its background load is being measured.
    case waitingForQuiet

    /// Granted; it has the machine to itself.
    case runningAlone
}

public struct PanelHeader: Sendable, Equatable {
    public let glyph: HeaderGlyph
    public let summary: String
}

/// The strip at the top of a card that names the job's session.
public struct SessionStrip: Sendable, Equatable {
    /// The session ID's first 8 characters.
    public let shortId: String

    /// The project folder name, or empty.
    public let project: String

    /// The full working directory, for the project's tooltip.
    public let projectPath: String

    /// Live session usage, `also running` on a queued card of a session that runs a job, or
    /// empty when the session is not registered.
    public let usage: String
}

/// One job row: an active lease or a queued request.
public struct JobRow: Sendable, Equatable, Identifiable {
    /// The lease ID.
    public let id: Int64

    /// The command, from the lease's label.
    public let command: String

    public let session: SessionStrip

    /// Waiting in the queue rather than running.
    public let isQueued: Bool
}

public struct JobGroup: Sendable, Equatable, Identifiable {
    public let kind: JobGroupKind
    public let heading: String
    public let jobs: [JobRow]

    public var id: JobGroupKind {
        kind
    }
}

/// The plain `Idle · N  ids  usage` line after the groups.
public struct IdleLine: Sendable, Equatable {
    /// For example `Idle · 2`.
    public let label: String

    /// Short session IDs separated by two spaces.
    public let ids: String

    /// Live usage, shown only when exactly one session is idle.
    public let usage: String
}

/// Everything the panel header and job list show for one poll, built by a pure function so
/// that every string is testable. The Windows tray app's `FlyoutSnapshot` without what later
/// milestones add: card details, stops, recent runs and the first-run card.
public struct PanelSnapshot: Sendable, Equatable {
    public let header: PanelHeader

    /// Benchmark, Running and Waiting, in that order, each only when it has jobs.
    public let groups: [JobGroup]

    /// The idle line, or `nil` when no session is idle.
    public let idle: IdleLine?

    /// The "No jobs running or waiting." line: the service runs and has no jobs.
    public let showNothing: Bool

    /// The live benchmark's phase, or `nil` without one.
    public let benchmark: BenchmarkPhase?

    /// The jobs come from the last state allotd served before it went away; they show dimmed.
    public let isLastKnown: Bool

    /// - Parameters:
    ///   - status: The service status of the latest poll.
    ///   - state: The latest state, or the last known one while allotd is not running.
    public static func from(status: ServiceStatus, state: UiState?) -> PanelSnapshot {
        let lastKnown = status != .running
        var sessions: [String: UiSession] = [:]
        var ordered: [UiSession] = []
        for session in state?.sessions ?? [] where sessions[session.session] == nil {
            sessions[session.session] = session
            ordered.append(session)
        }
        let active = state?.active ?? []
        let runningSessions = Set(active.map(\.lease.holder.session))
        let waitingSessions = Set((state?.queue ?? []).map(\.lease.holder.session))

        var benchmark: [JobRow] = []
        var running: [JobRow] = []
        var waiting: [JobRow] = []
        var benchmarkRuns = false
        for job in active {
            let lease = job.lease
            let strip = strip(lease.holder.session, lease.cwd, sessions, alsoRunning: false)
            let row = JobRow(id: lease.id, command: lease.label, session: strip, isQueued: false)
            if isBenchmark(lease) {
                benchmark.append(row)
                benchmarkRuns = true
            } else {
                running.append(row)
            }
        }
        var firstQueuedBenchmark: QueuedJob?
        for job in state?.queue ?? [] {
            let lease = job.lease
            let alsoRunning = runningSessions.contains(lease.holder.session)
            let strip = strip(lease.holder.session, lease.cwd, sessions, alsoRunning: alsoRunning)
            let row = JobRow(id: lease.id, command: lease.label, session: strip, isQueued: true)
            if isBenchmark(lease) {
                benchmark.append(row)
                firstQueuedBenchmark = firstQueuedBenchmark ?? job
            } else {
                waiting.append(row)
            }
        }

        var groups: [JobGroup] = []
        if !benchmark.isEmpty {
            groups.append(JobGroup(kind: .benchmark, heading: "Benchmark", jobs: benchmark))
        }
        if !running.isEmpty {
            groups.append(JobGroup(kind: .running, heading: "Running · \(running.count)",
                                   jobs: running))
        }
        if !waiting.isEmpty {
            groups.append(JobGroup(kind: .waiting, heading: "Waiting · \(waiting.count)",
                                   jobs: waiting))
        }

        let benchmarkPhase: BenchmarkPhase? = benchmarkRuns
            ? .runningAlone
            : firstQueuedBenchmark.map(phase(of:))
        return PanelSnapshot(
            header: header(
                status, running: running.count, waiting: waiting.count, phase: benchmarkPhase),
            groups: groups,
            idle: idleLine(ordered, busy: runningSessions.union(waitingSessions)),
            showNothing: !lastKnown && groups.isEmpty,
            benchmark: benchmarkPhase,
            isLastKnown: lastKnown)
    }

    private static func header(
        _ status: ServiceStatus,
        running: Int,
        waiting: Int,
        phase: BenchmarkPhase?
    ) -> PanelHeader {
        switch status {
        case .starting:
            return PanelHeader(glyph: .error, summary: "Starting service…")
        case .stopped:
            return PanelHeader(glyph: .error, summary: "Service stopped")
        case .incompatible:
            return PanelHeader(glyph: .error, summary: "Service needs an update")
        case .running:
            break
        }

        let glyph: HeaderGlyph = running > 0 || phase == .runningAlone ? .busy : .idle
        let summary = switch phase {
        case .draining where running > 0: "\(running) running · benchmark waiting"
        case .draining: "Benchmark waiting"
        case .waitingForQuiet: "Benchmark waiting for quiet"
        case .runningAlone: "Benchmark running alone"
        case nil: counts(running: running, waiting: waiting)
        }
        return PanelHeader(glyph: glyph, summary: summary)
    }

    private static func counts(running: Int, waiting: Int) -> String {
        var parts: [String] = []
        if running > 0 {
            parts.append("\(running) running")
        }
        if waiting > 0 {
            parts.append("\(waiting) waiting")
        }
        return parts.isEmpty ? "Idle" : parts.joined(separator: " · ")
    }

    // A queued benchmark either drains the machine or, once it is at the head and drained,
    // measures the background load.
    private static func phase(of job: QueuedJob) -> BenchmarkPhase {
        switch job.verdict.benchmark {
        case .measuring, .noisy: .waitingForQuiet
        case .draining, nil: .draining
        }
    }

    private static func idleLine(_ sessions: [UiSession], busy: Set<String>) -> IdleLine? {
        let idle = sessions.filter { !busy.contains($0.session) }
        guard !idle.isEmpty else {
            return nil
        }
        return IdleLine(
            label: "Idle · \(idle.count)",
            ids: idle.map { Format.shortId($0.session) }.joined(separator: "  "),
            usage: idle.count == 1 ? Format.usage(idle[0].usage) : "")
    }

    private static func isBenchmark(_ lease: Lease) -> Bool {
        lease.kind == .benchmarkExclusive
    }

    // A queued card of a session that also runs a job says so instead of repeating the
    // session's usage, which its running card already shows.
    private static func strip(
        _ key: String,
        _ jobCwd: String,
        _ sessions: [String: UiSession],
        alsoRunning: Bool
    ) -> SessionStrip {
        let session = sessions[key]
        let cwd = session?.cwd.flatMap { $0.isEmpty ? nil : $0 } ?? jobCwd
        let usage = alsoRunning ? "also running" : session.map { Format.usage($0.usage) } ?? ""
        return SessionStrip(
            shortId: Format.shortId(key),
            project: Format.project(cwd),
            projectPath: cwd,
            usage: usage)
    }
}
