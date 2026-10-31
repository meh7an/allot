import AllotContract

/// What the 2 pt edge bar along a card's bottom shows.
public enum EdgeBarTone: Sendable, Equatable {
    /// Green: elapsed against the estimate.
    case running

    /// Amber and full: past the estimate.
    case over

    /// Amber: waited against the expected wait.
    case waiting

    /// Amber dashes across the whole card: the wait has no estimate.
    case waitingDashed

    /// Gray and still: a stopped job, or last known state while the service is down.
    case frozen

    /// No bar and no track: a removed request.
    case none
}

public enum CardAction: Sendable, Equatable {
    /// A 32 pt spacer keeps the layout.
    case none

    /// The Kill icon button of a running job.
    case kill

    /// The red Kill? button that confirms a kill.
    case confirm

    /// The Remove text button of a queued request.
    case remove
}

/// The job row of a card at one moment (handoff section 4).
public struct JobCard: Sendable, Equatable {
    // A queued job this close to its start reads "starts next".
    private static let startsNextSecs: UInt64 = 60

    /// Two-letter initials of the process, or `#N` for a queued job.
    public internal(set) var tile: String

    /// The tile is the dashed queue position.
    public internal(set) var isQueued: Bool

    /// The process executable, whose app icon replaces the initials when it has one; `nil`
    /// for queued jobs and unknown paths.
    public internal(set) var exePath: String?

    public internal(set) var chip: String
    public internal(set) var chipTooltip: String

    /// The chip is the solid purple Benchmark chip.
    public internal(set) var isBenchmark: Bool

    /// The semibold start of the detail line (`Stopped`, `Removed`, `Finished`); empty
    /// otherwise.
    public internal(set) var strong = ""

    /// The detail line's left part, after ``strong``.
    public internal(set) var detail: String

    /// The detail's tooltip, or empty for none.
    public internal(set) var detailTooltip = ""

    /// The detail reads in red: the Kill? confirm, or a refusal.
    public internal(set) var detailIsDanger = false

    /// The detail line's right part.
    public internal(set) var time: String

    /// The time reads in amber: past the estimate, or waiting.
    public internal(set) var timeIsWarning = false

    /// How much of the edge bar is filled, from 0 to 1.
    public internal(set) var barFraction: Double

    public internal(set) var barTone: EdgeBarTone

    /// The control at the row's right.
    public internal(set) var action: CardAction

    /// The action's accessible name, for example `Kill cargo`.
    public internal(set) var actionName = ""

    /// The job has ended (stopped, removed, finished, ended by allotd): the tile, command and
    /// chip show at 50%.
    public internal(set) var isEnded = false

    /// The command is struck through: stopped or removed, not finished.
    public internal(set) var isStruck = false

    /// The detail line's leading icon; with one, the line wraps instead of truncating.
    public internal(set) var icon = DetailIcon.none

    /// A benchmark's Drain / Quiet / Run stepper, or `nil` for none.
    public internal(set) var steps: BenchmarkSteps?

    /// What VoiceOver announces when a stop ends or is refused, for example
    /// `Stopped cargo · Claude was notified`; empty otherwise.
    public internal(set) var announcement = ""

    /// - Parameters:
    ///   - job: The running job.
    ///   - nowMs: The moment to show, in the daemon's clock.
    ///   - lastKnown: The job comes from the last state allotd served before it went away.
    ///   - stop: A Kill under way or done in this panel, or `nil`; ignored for last known state.
    public static func forActive(
        _ job: ActiveJob,
        nowMs: Int64,
        lastKnown: Bool,
        stop: StopView? = nil
    ) -> JobCard {
        let card = activeCard(job, nowMs: nowMs, lastKnown: lastKnown, stop: stop)
        guard card.isBenchmark else {
            return card
        }
        let lease = job.lease
        let elapsed = seconds(nowMs - (lease.grantedAtMs ?? lease.requestedAtMs))
        let status = BenchmarkStatus.forActive(job, elapsed: elapsed)
        return benchmark(
            card, status, lastKnown: lastKnown, stop: stop?.kind == .kill ? stop : nil)
    }

    /// A benchmark that finished on its own: the green check, the run time and its background
    /// noise, every step complete, and no action.
    public static func forFinished(_ run: RecentRun, noise: NoiseReport) -> JobCard {
        let status = BenchmarkStatus.forFinished(run, noise: noise)
        var card = JobCard(
            tile: Format.initials(firstWord(run.command)),
            isQueued: false,
            chip: "Benchmark",
            chipTooltip: "Benchmark",
            isBenchmark: true,
            detail: status.text,
            time: "",
            barFraction: 0,
            barTone: .none,
            action: .none)
        card.icon = status.icon
        card.steps = status.steps
        return card
    }

    /// - Parameters:
    ///   - job: The waiting request.
    ///   - nowMs: The moment to show, in the daemon's clock.
    ///   - afterBenchmark: A benchmark runs or waits ahead of it.
    ///   - lastKnown: The job comes from the last state allotd served before it went away.
    ///   - stop: A Remove under way or done in this panel, or `nil`; ignored for last known
    ///     state.
    ///   - othersActive: Another lease is active, which a queued benchmark drains.
    public static func forQueued(
        _ job: QueuedJob,
        nowMs: Int64,
        afterBenchmark: Bool,
        lastKnown: Bool,
        stop: StopView? = nil,
        othersActive: Bool = false
    ) -> JobCard {
        let card = queuedCard(
            job, nowMs: nowMs, afterBenchmark: afterBenchmark, lastKnown: lastKnown, stop: stop)
        guard card.isBenchmark else {
            return card
        }
        return benchmark(
            card,
            BenchmarkStatus.forQueued(job, othersActive: othersActive),
            lastKnown: lastKnown,
            stop: stop?.kind == .remove ? stop : nil)
    }

    private static func activeCard(
        _ job: ActiveJob,
        nowMs: Int64,
        lastKnown: Bool,
        stop: StopView?
    ) -> JobCard {
        let lease = job.lease
        let process = job.process
        let started = lease.grantedAtMs ?? lease.requestedAtMs
        let elapsed = seconds(nowMs - started)
        let estimate = Int64(lease.profile.estSecs)
        let over = estimate > 0 && elapsed > estimate
        let progress = estimate > 0 ? fraction(elapsed, estimate) : 0
        let time = estimate > 0
            ? "\(Format.duration(elapsed)) / ~\(Format.duration(estimate))"
            : Format.duration(elapsed)
        let prefix = process.map { "\($0.name) \($0.pid) · " } ?? ""
        let badge = chip(of: lease)
        let tile = Format.initials(process?.name ?? firstWord(lease.label))

        if lastKnown {
            return JobCard(
                tile: tile,
                isQueued: false,
                exePath: process?.exe,
                chip: badge.label,
                chipTooltip: badge.tooltip,
                isBenchmark: badge.isBenchmark,
                detail: "\(prefix)was at \(time)",
                time: "",
                barFraction: progress,
                barTone: .frozen,
                action: .none)
        }

        let detail = prefix + Format.reservation(lease.profile)
        let name = process?.name ?? lease.label
        var card = JobCard(
            tile: tile,
            isQueued: false,
            exePath: process?.exe,
            chip: badge.label,
            chipTooltip: badge.tooltip,
            isBenchmark: badge.isBenchmark,
            detail: detail,
            detailTooltip: detail,
            time: over ? "\(Format.duration(elapsed - estimate)) over" : time,
            timeIsWarning: over,
            barFraction: over ? 1 : progress,
            barTone: over ? .over : .running,
            action: .kill,
            actionName: "Kill \(name)")
        if let outside = endedOutside(
            card, stop: stop, started: started, estimate: estimate, nowMs: nowMs)
        {
            return outside
        }
        guard let stop, stop.kind == .kill else {
            return card
        }

        switch stop.phase {
        case .confirming:
            let confirm = "Stops \(process?.name ?? "the command") and its child processes"
            card.detail = confirm
            card.detailTooltip = confirm
            card.detailIsDanger = true
            card.time = "\(stop.secondsLeft)s"
            card.timeIsWarning = false
            card.action = .confirm
            card.actionName = "Confirm kill \(name)"
        case .sending:
            card.detail = process.map { "Stopping \($0.name)…" } ?? "Stopping…"
            card.detailTooltip = ""
            card.time = ""
            card.timeIsWarning = false
            card.action = .none
            card.actionName = ""
        case .failed:
            card.detail = "Not stopped: \(stop.message)"
            card.detailTooltip = card.detail
            card.detailIsDanger = true
            card.announcement = "\(name) not stopped: \(stop.message)"
        case .ended:
            let ran = seconds((stop.endedAtMs ?? nowMs) - started)
            card.strong = "Stopped"
            card.detail = notified(stop.message)
            card.detailTooltip = ""
            card.time = "at \(Format.duration(ran))"
            card.timeIsWarning = false
            card.barFraction = estimate > 0 ? fraction(ran, estimate) : 0
            card.barTone = .frozen
            card.action = .none
            card.actionName = ""
            card.isEnded = true
            card.isStruck = true
            card.announcement = "Stopped \(name)\(notified(stop.message))"
        }
        return card
    }

    private static func queuedCard(
        _ job: QueuedJob,
        nowMs: Int64,
        afterBenchmark: Bool,
        lastKnown: Bool,
        stop: StopView?
    ) -> JobCard {
        let lease = job.lease
        let waited = seconds(nowMs - lease.requestedAtMs)
        let left = job.verdict.waitEstimateSecs
        let badge = chip(of: lease)

        let time: String
        let filled: Double
        var tone = EdgeBarTone.waiting
        if afterBenchmark {
            time = "\(Format.duration(waited)) · after benchmark"
            filled = 1
            tone = .waitingDashed
        } else if job.position == 1 && (job.verdict.fits || (left ?? .max) <= startsNextSecs) {
            time = left.map { "starts next · ~\(Format.minutes($0))" } ?? "starts next"
            filled = left.map { fraction(waited, waited + Int64($0)) } ?? 1
        } else if let left {
            let total = waited + Int64(left)
            time = "\(Format.duration(waited)) / ~\(Format.minutes(UInt64(total)))"
            filled = fraction(waited, total)
        } else {
            time = "\(Format.duration(waited)) · depends on queue"
            filled = 1
            tone = .waitingDashed
        }

        var card = JobCard(
            tile: "#\(job.position)",
            isQueued: true,
            chip: badge.label,
            chipTooltip: badge.tooltip,
            isBenchmark: badge.isBenchmark,
            detail: Format.reservation(lease.profile),
            time: time,
            timeIsWarning: true,
            barFraction: filled,
            barTone: lastKnown ? .frozen : tone,
            action: lastKnown ? .none : .remove,
            actionName: lastKnown ? "" : "Remove \(lease.label) from queue")
        guard !lastKnown, let stop, stop.kind == .remove else {
            return card
        }

        switch stop.phase {
        case .sending:
            card.detail = "Removing…"
            card.action = .none
            card.actionName = ""
        case .failed:
            card.detail = "Not removed: \(stop.message)"
            card.detailTooltip = card.detail
            card.detailIsDanger = true
            card.announcement = "\(lease.label) not removed: \(stop.message)"
        case .ended:
            card.strong = "Removed"
            card.detail = notified(stop.message)
            card.time = ""
            card.timeIsWarning = false
            card.barFraction = 0
            card.barTone = .none
            card.action = .none
            card.actionName = ""
            card.isEnded = true
            card.isStruck = true
            card.announcement = "Removed \(lease.label)\(notified(stop.message))"
        case .confirming:
            break
        }
        return card
    }

    // A benchmark shows its progress on the stepper instead of an edge bar, and its status line
    // instead of the reservation. While a stop is under way the stepper stays and the line is
    // the stop's; once stopped or removed, and in last known state, only the plain line is
    // left, without a time, as in the prototype.
    private static func benchmark(
        _ card: JobCard,
        _ status: BenchmarkStatus,
        lastKnown: Bool,
        stop: StopView?
    ) -> JobCard {
        var card = card
        card.barFraction = 0
        card.barTone = .none
        if card.isEnded {
            card.time = ""
            return card
        }
        if lastKnown {
            return card
        }
        if let stop {
            if stop.phase != .confirming {
                card.time = ""
            }
            card.timeIsWarning = false
            card.steps = status.steps
            return card
        }
        card.detail = status.text
        card.detailTooltip = ""
        card.time = ""
        card.timeIsWarning = false
        card.icon = status.icon
        card.steps = status.steps
        return card
    }

    /// A job that ended without this panel stopping it, held like Stopped: Finished with the
    /// green check and its run time, not struck through; a kill from elsewhere reads as a stop;
    /// allotd's own end gives its reason.
    ///
    /// - Returns: The held card, or `nil` when `stop` is no such end.
    private static func endedOutside(
        _ card: JobCard,
        stop: StopView?,
        started: Int64,
        estimate: Int64,
        nowMs: Int64
    ) -> JobCard? {
        guard let stop, stop.phase == .ended,
              [.finished, .killedOutside, .endedByAllotd].contains(stop.kind)
        else {
            return nil
        }
        let ran = seconds((stop.endedAtMs ?? nowMs) - started)
        var ended = card
        ended.detailTooltip = ""
        ended.time = "at \(Format.duration(ran))"
        ended.timeIsWarning = false
        ended.barFraction = estimate > 0 ? fraction(ran, estimate) : 0
        ended.barTone = .frozen
        ended.action = .none
        ended.actionName = ""
        ended.isEnded = true
        switch stop.kind {
        case .finished:
            ended.strong = "Finished"
            ended.detail = " · \(Format.duration(ran))"
            ended.time = ""
            ended.icon = .check
        case .killedOutside:
            ended.strong = "Stopped outside the panel"
            ended.detail = notified("")
            ended.isStruck = true
        default:
            ended.strong = "Ended by allotd"
            ended.detail = stop.message.isEmpty ? "" : " · \(stop.message)"
            ended.detailTooltip = stop.message
        }
        return ended
    }

    private static func notified(_ note: String) -> String {
        note.isEmpty ? " · Claude was notified" : " · \(note) · Claude was notified"
    }

    private static func chip(of lease: Lease) -> Chip {
        if lease.kind == .benchmarkExclusive {
            return Chip(label: "Benchmark", tooltip: "Benchmark", isBenchmark: true)
        }
        let label = switch lease.class {
        case .cpuHeavy: "CPU heavy"
        case .gpuHeavy: "GPU heavy"
        case .mixed: "Mixed"
        case .light: "Light"
        case nil: "Custom"
        }
        return Chip(
            label: label,
            tooltip: lease.class == nil ? "Custom reservation" : label,
            isBenchmark: false)
    }

    private static func seconds(_ milliseconds: Int64) -> Int64 {
        max(0, milliseconds / 1000)
    }

    private static func fraction(_ part: Int64, _ whole: Int64) -> Double {
        whole <= 0 ? 1 : min(1, max(0, Double(part) / Double(whole)))
    }

    private static func firstWord(_ command: String) -> String {
        let trimmed = command.drop { $0 == " " }
        return String(trimmed.prefix { $0 != " " })
    }

    private struct Chip {
        let label: String
        let tooltip: String
        let isBenchmark: Bool
    }
}
