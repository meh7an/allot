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

/// One card: an active lease, a queued request, or a benchmark that finished a moment ago.
public struct JobRow: Sendable, Equatable, Identifiable {
    /// The lease ID; a finished benchmark keeps its lease's, so its card replaces the running
    /// one in place.
    public let id: Int64

    /// The command, from the lease's label.
    public let command: String

    public let session: SessionStrip

    /// Waiting in the queue rather than running.
    public let isQueued: Bool

    /// The job row's text and state at the snapshot's moment.
    public let card: JobCard

    /// Stopped or removed in this panel and still showing its final state, or a finished
    /// benchmark; it no longer counts in the summary.
    public let isEnding: Bool
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

/// One row of the Recently finished card.
public struct RecentLine: Sendable, Equatable, Identifiable {
    /// Its place in the list, newest first.
    public let id: Int

    /// The command as it ran.
    public let command: String

    /// The session's short ID, or empty for runs recorded before sessions were.
    public let shortId: String

    /// Run time and age, for example `4:52 · 4 min ago`.
    public let meta: String
}

/// Everything the panel header and job list show for one poll, built by a pure function so
/// that every string is testable. The Windows tray app's `FlyoutSnapshot` without the first-run
/// card, which comes with the service-down and first-run views.
public struct PanelSnapshot: Sendable, Equatable {
    // How long a benchmark that finished on its own keeps its card.
    private static let finishedHoldMs: Int64 = 60_000

    public let header: PanelHeader

    /// Benchmark, Running and Waiting, in that order, each only when it has jobs.
    public let groups: [JobGroup]

    /// The idle line, or `nil` when no session is idle.
    public let idle: IdleLine?

    /// Recently finished runs, newest first; only while no job runs or waits.
    public let recent: [RecentLine]

    /// The "No jobs running or waiting." line: the service runs, and there are no jobs and no
    /// recent runs.
    public let showNothing: Bool

    /// The live benchmark's phase, or `nil` without one.
    public let benchmark: BenchmarkPhase?

    /// The jobs come from the last state allotd served before it went away; they show dimmed
    /// and without actions.
    public let isLastKnown: Bool

    /// - Parameters:
    ///   - status: The service status of the latest poll.
    ///   - state: The latest state, or the last known one while allotd is not running.
    ///   - jobs: The jobs to show instead of the state's, with ended stops put back
    ///     (``JobStops/merge(_:)``); the state's own when `nil`.
    ///   - stops: Kills and removals of this panel by lease; ended ones show Stopped or Removed
    ///     and no longer count. Ignored for last known state.
    ///   - nowMs: The moment times are shown for, in the daemon's clock; the state's own time
    ///     when `nil`. Last known state always shows its own time.
    public static func from(
        status: ServiceStatus,
        state: UiState?,
        jobs: ShownJobs? = nil,
        stops: [Int64: StopView] = [:],
        nowMs: Int64? = nil
    ) -> PanelSnapshot {
        let lastKnown = status != .running
        let stops = lastKnown ? [:] : stops
        let now = lastKnown
            ? state?.generatedAtMs ?? 0
            : nowMs ?? state?.generatedAtMs ?? 0
        var sessions: [String: UiSession] = [:]
        var ordered: [UiSession] = []
        for session in state?.sessions ?? [] where sessions[session.session] == nil {
            sessions[session.session] = session
            ordered.append(session)
        }
        let shown = jobs ?? state.map(ShownJobs.init) ?? ShownJobs(active: [], queue: [])
        let isEnding = { (lease: Lease) in stops[lease.id]?.phase == .ended }
        let runningSessions = Set(shown.active.map(\.lease.holder.session))
        // A stopped or removed benchmark no longer holds anything back.
        var benchmarkAhead = shown.active.contains { job in
            isBenchmark(job.lease) && !isEnding(job.lease)
        }
        let othersActive = shown.active.contains { !isEnding($0.lease) }

        var benchmark: [JobRow] = []
        var running: [JobRow] = []
        var waiting: [JobRow] = []
        for job in shown.active {
            let lease = job.lease
            let row = JobRow(
                id: lease.id,
                command: lease.label,
                session: strip(lease.holder.session, lease.cwd, sessions, alsoRunning: false),
                isQueued: false,
                card: .forActive(job, nowMs: now, lastKnown: lastKnown, stop: stops[lease.id]),
                isEnding: isEnding(lease))
            if isBenchmark(lease) {
                benchmark.append(row)
            } else {
                running.append(row)
            }
        }
        for job in shown.queue {
            let lease = job.lease
            let alsoRunning = runningSessions.contains(lease.holder.session)
            let card = JobCard.forQueued(
                job,
                nowMs: now,
                afterBenchmark: benchmarkAhead && !isBenchmark(lease),
                lastKnown: lastKnown,
                stop: stops[lease.id],
                othersActive: othersActive)
            let row = JobRow(
                id: lease.id,
                command: lease.label,
                session: strip(lease.holder.session, lease.cwd, sessions, alsoRunning: alsoRunning),
                isQueued: true,
                card: card,
                isEnding: isEnding(lease))
            if isBenchmark(lease) {
                benchmark.append(row)
                benchmarkAhead = benchmarkAhead || !row.isEnding
            } else {
                waiting.append(row)
            }
        }
        benchmark += finished(state, shown, sessions, now: now)

        var groups: [JobGroup] = []
        if !benchmark.isEmpty {
            groups.append(JobGroup(kind: .benchmark, heading: "Benchmark", jobs: benchmark))
        }
        if !running.isEmpty {
            groups.append(
                JobGroup(kind: .running, heading: counted("Running", running), jobs: running))
        }
        if !waiting.isEmpty {
            groups.append(
                JobGroup(kind: .waiting, heading: counted("Waiting", waiting), jobs: waiting))
        }

        let benchmarkPhase = phase(of: benchmark, shown)
        let recent = groups.isEmpty ? recentLines(state, now: now) : []
        return PanelSnapshot(
            header: header(
                status, running: alive(running), waiting: alive(waiting), phase: benchmarkPhase),
            groups: groups,
            idle: idleLine(ordered, state),
            recent: recent,
            showNothing: !lastKnown && groups.isEmpty && recent.isEmpty,
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

    // An active benchmark runs alone; a queued one either drains the machine or, once it is at
    // the head and drained, measures the background load.
    private static func phase(of rows: [JobRow], _ shown: ShownJobs) -> BenchmarkPhase? {
        let alive = rows.filter { !$0.isEnding }
        guard let first = alive.first else {
            return nil
        }
        let activeIds = Set(shown.active.map(\.lease.id))
        if alive.contains(where: { activeIds.contains($0.id) }) {
            return .runningAlone
        }
        let verdict = shown.queue.first { $0.lease.id == first.id }?.verdict
        switch verdict?.benchmark {
        case .measuring, .noisy: return .waitingForQuiet
        case .draining, nil: return .draining
        }
    }

    private static func idleLine(_ sessions: [UiSession], _ state: UiState?) -> IdleLine? {
        guard let state else {
            return nil
        }
        let busy = Set(state.active.map(\.lease.holder.session))
            .union(state.queue.map(\.lease.holder.session))
        let idle = sessions.filter { !busy.contains($0.session) }
        guard !idle.isEmpty else {
            return nil
        }
        return IdleLine(
            label: "Idle · \(idle.count)",
            ids: idle.map { Format.shortId($0.session) }.joined(separator: "  "),
            usage: idle.count == 1 ? Format.usage(idle[0].usage) : "")
    }

    // Benchmarks that finished on their own in the last finishedHoldMs, from allotd's recent
    // runs, so the card survives an app restart. A killed one already showed Stopped instead.
    private static func finished(
        _ state: UiState?,
        _ shown: ShownJobs,
        _ sessions: [String: UiSession],
        now: Int64
    ) -> [JobRow] {
        let live = Set(shown.active.map(\.lease.id)).union(shown.queue.map(\.lease.id))
        return (state?.recent ?? []).compactMap { run in
            guard let noise = run.noise, let id = run.leaseId,
                  run.endCause == nil || run.endCause == .released,
                  now - run.endedAtMs < finishedHoldMs,
                  !live.contains(id)
            else {
                return nil
            }
            return JobRow(
                id: id,
                command: run.command,
                session: strip(run.session ?? "", run.cwd, sessions, alsoRunning: false),
                isQueued: false,
                card: .forFinished(run, noise: noise),
                isEnding: true)
        }
    }

    private static func recentLines(_ state: UiState?, now: Int64) -> [RecentLine] {
        (state?.recent ?? []).enumerated().map { index, run in
            let ran = Format.duration((run.endedAtMs - run.startedAtMs) / 1000)
            let ago = Format.ago(max(0, now - run.endedAtMs) / 1000)
            return RecentLine(
                id: index,
                command: run.command,
                shortId: run.session.map(Format.shortId) ?? "",
                meta: "\(ran) · \(ago)")
        }
    }

    // While every card in a group shows Stopped or Removed, the heading keeps their number
    // until they collapse, as the handoff prototype does.
    private static func counted(_ label: String, _ rows: [JobRow]) -> String {
        let live = alive(rows)
        return "\(label) · \(live > 0 ? live : rows.count)"
    }

    private static func alive(_ rows: [JobRow]) -> Int {
        rows.filter { !$0.isEnding }.count
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
