import AllotContract

/// Where the Drain / Quiet / Run stepper stands.
public enum BenchmarkStep: Sendable, Equatable, Comparable {
    case drain
    case quiet
    case run

    /// The run finished; every step is complete.
    case done
}

/// The leading icon of a card's detail line.
public enum DetailIcon: Sendable, Equatable {
    /// No icon; the line truncates with an ellipsis.
    case none

    /// The purple timer of a benchmark in progress; the line wraps.
    case timer

    /// The green check of a finished benchmark; the line wraps.
    case check
}

/// The stepper under a benchmark's status line.
public struct BenchmarkSteps: Sendable, Equatable {
    /// Ticks of the Quiet step, one per second of a 10 s quiet window.
    public static let tickCount = 10

    /// The step in progress, or ``BenchmarkStep/done``.
    public let current: BenchmarkStep

    /// Filled ticks of the Quiet step, out of ``tickCount``.
    public let quietTicks: Int

    /// How much of the Run step is filled, from 0 to 1.
    public let runFraction: Double
}

/// A benchmark card's status line and stepper (handoff section 4, Benchmark).
public struct BenchmarkStatus: Sendable, Equatable {
    private static let notQuiet = "not quiet at start · "

    private static let draining = BenchmarkStatus(
        text: "Draining · waiting for running jobs to finish",
        icon: .timer,
        steps: BenchmarkSteps(current: .drain, quietTicks: 0, runFraction: 0))

    public let text: String
    public let icon: DetailIcon
    public let steps: BenchmarkSteps

    /// - Parameters:
    ///   - job: The benchmark, granted and running alone.
    ///   - elapsed: Seconds since it was granted.
    public static func forActive(_ job: ActiveJob, elapsed: Int64) -> BenchmarkStatus {
        let estimate = Int64(job.lease.profile.estSecs)
        let time = if estimate > 0 && elapsed > estimate {
            "\(Format.duration(elapsed - estimate)) over"
        } else if estimate > 0 {
            "\(Format.duration(elapsed)) / ~\(Format.duration(estimate))"
        } else {
            Format.duration(elapsed)
        }
        let forced = job.quietAtStart == false ? notQuiet : ""
        let fraction = estimate > 0 ? min(1, max(0, Double(elapsed) / Double(estimate))) : 0
        return BenchmarkStatus(
            text: "Running alone · \(forced)\(time)",
            icon: .timer,
            steps: BenchmarkSteps(
                current: .run, quietTicks: BenchmarkSteps.tickCount, runFraction: fraction))
    }

    /// - Parameters:
    ///   - job: The benchmark, still queued. Only the head of the queue carries progress.
    ///   - othersActive: Another lease is still active, so a benchmark without progress waits
    ///     for it to drain; otherwise it waits for room, with allotd's first reason.
    public static func forQueued(_ job: QueuedJob, othersActive: Bool) -> BenchmarkStatus {
        switch job.verdict.benchmark {
        case .measuring(let measuring):
            let cpu = Format.whole(Double(measuring.cpuAvgPct))
            return BenchmarkStatus(
                text: "Waiting for quiet · \(measuring.measuredSecs) / \(measuring.neededSecs) s"
                    + " · background \(cpu)% CPU",
                icon: .timer,
                steps: BenchmarkSteps(current: .quiet, quietTicks: ticks(measuring),
                                      runFraction: 0))
        // The window is full but too busy; allotd grants it anyway once the longest wait ends.
        case .noisy(let noisy):
            let cpu = Format.whole(Double(noisy.cpuAvgPct))
            let limit = Format.whole(Double(noisy.cpuLimitPct))
            return BenchmarkStatus(
                text: "Not quiet · background \(cpu)% CPU (limit \(limit)%)"
                    + " · starting anyway \(within(noisy.grantAnywayInSecs))",
                icon: .timer,
                steps: BenchmarkSteps(
                    current: .quiet, quietTicks: BenchmarkSteps.tickCount, runFraction: 0))
        case .draining:
            return draining
        case nil where othersActive:
            return draining
        // Drained, but its own reservation does not fit (seen live: 16 cores, 12.4 free).
        case nil:
            let reasons = job.verdict.reasons
            return BenchmarkStatus(
                text: reasons.isEmpty ? "Waiting for room" : "Waiting for room · \(reasons[0])",
                icon: .timer,
                steps: BenchmarkSteps(current: .drain, quietTicks: 0, runFraction: 0))
        }
    }

    /// - Parameters:
    ///   - run: The finished benchmark run.
    ///   - noise: Its background load.
    public static func forFinished(_ run: RecentRun, noise: NoiseReport) -> BenchmarkStatus {
        let ran = max(0, (run.endedAtMs - run.startedAtMs) / 1000)
        let forced = noise.quietAtStart ? "" : notQuiet
        let average = Format.oneDecimal(Double(noise.avgCpuPct))
        let peak = Format.oneDecimal(Double(noise.peakCpuPct))
        return BenchmarkStatus(
            text: "Finished in \(Format.duration(ran)) · \(forced)background noise avg "
                + "\(average)%, peak \(peak)% CPU",
            icon: .check,
            steps: BenchmarkSteps(
                current: .done, quietTicks: BenchmarkSteps.tickCount, runFraction: 1))
    }

    // The window may be configured longer or shorter than 10 s; the ticks show its share.
    private static func ticks(_ measuring: BenchmarkProgress.Measuring) -> Int {
        guard measuring.neededSecs > 0 else {
            return BenchmarkSteps.tickCount
        }
        let share = measuring.measuredSecs * UInt64(BenchmarkSteps.tickCount)
            / measuring.neededSecs
        return Int(min(UInt64(BenchmarkSteps.tickCount), share))
    }

    private static func within(_ seconds: UInt64) -> String {
        seconds == 0 ? "now" : "in \(Format.duration(Int64(seconds)))"
    }
}
