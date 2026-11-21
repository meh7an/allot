import AllotContract
import Foundation

/// The live sentence at the end of each group with numbers (DESIGN.md D10), built from the
/// values on screen or from allotd's fit preview.
public enum LiveText {
    public static let serviceStopped = "Can't work this out while the service is stopped."

    public static let loweringOff =
        "Requests are reserved as declared, or higher when past runs used more."

    public static let noneFit = "None fit"

    // The example in the learned-estimate sentence: a command asking for 8 cores that used 3.
    private static let exampleAsked = 8.0
    private static let exampleUsed = 3.0

    /// "Keeps 2.4 of 16 cores, 4.8 of 32 GB memory, 15 % of the GPU and 1.2 of 8 GB video
    /// memory free." The GPU parts are left out without a dedicated adapter.
    public static func headroom(_ kept: KeptFree, number: (Double) -> String) -> String {
        var parts = [
            "\(number(kept.cores)) of \(number(Double(kept.totalCores))) cores",
            "\(gigabytes(kept.ramMb, number)) of \(gigabytes(Double(kept.totalRamMb), number)) "
                + "GB memory",
        ]
        if let gpu = kept.gpuPct {
            parts.append("\(number(gpu)) % of the GPU")
        }
        if let vram = kept.vramMb, let totalVram = kept.totalVramMb {
            parts.append(
                "\(gigabytes(vram, number)) of \(gigabytes(Double(totalVram), number)) GB video "
                    + "memory")
        }
        return "Keeps \(SettingsText.joined(parts)) free."
    }

    /// What learning from past runs does to an example command, with the values on screen.
    ///
    /// - Parameters:
    ///   - lower: Whether lowering is on.
    ///   - minRuns: Runs before lowering.
    ///   - marginPct: The safety margin.
    public static func estimates(
        lower: Bool,
        minRuns: Double,
        marginPct: Double,
        number: (Double) -> String
    ) -> String {
        guard lower else {
            return loweringOff
        }
        // Half away from zero, as the other panels round.
        let reserved =
            (exampleUsed * (1 + marginPct / 100) * 100).rounded(.toNearestOrAwayFromZero) / 100
        let runs = minRuns == 1 ? "run" : "\(number(minRuns)) runs"
        return "A command that asks for \(number(exampleAsked)) cores but used about "
            + "\(number(exampleUsed)) in its last \(runs) is reserved "
            + "\(cores(reserved, number)). Requests never go below 0.25 cores, 256 MB, or "
            + "their minimum cores, and run time never lowers."
    }

    /// A cell of What fits: "1 at full size, 2 more at 4 cores", "at least 99 at full size",
    /// or "None fit".
    public static func fits(_ fits: Fits, minCores: Double, number: (Double) -> String) -> String {
        if fits.full == 0 && fits.shrunk == 0 {
            return noneFit
        }
        // Counts stop at allotd's limit; the larger count then reads "at least 99".
        let capped = fits.more ? "at least " : ""
        var parts: [String] = []
        if fits.full > 0 {
            let full = fits.shrunk == 0 ? capped : ""
            parts.append("\(full)\(number(Double(fits.full))) at full size")
        }
        if fits.shrunk > 0 {
            let more = fits.full > 0 ? " more" : ""
            parts.append(
                "\(capped)\(number(Double(fits.shrunk)))\(more) at \(cores(minCores, number))")
        }
        return parts.joined(separator: ", ")
    }

    /// The caption under What fits.
    public static func fitsCaption(_ cpuBusyPct: Double, number: (Double) -> String) -> String {
        let busy = cpuBusyPct.rounded(.toNearestOrAwayFromZero)
        return "Based on this machine now (\(number(busy)) % CPU busy) and with every "
            + "lease ended."
    }

    /// "A benchmark starts once CPU and GPU stay at or below 10 % for 10 seconds. If that takes
    /// longer than 1 minute, it starts anyway, marked not quiet."
    public static func benchmarks(
        cpuPct: Double,
        gpuPct: Double,
        quietSecs: Double,
        maxWaitSecs: Double,
        number: (Double) -> String
    ) -> String {
        let cpu = quantityText(cpuPct, .percent, number: number)
        let gpu = quantityText(gpuPct, .percent, number: number)
        let loads = cpuPct == gpuPct
            ? "CPU and GPU stay at or below \(cpu)"
            : "CPU stays at or below \(cpu) and GPU at or below \(gpu)"
        let quiet = quantityText(quietSecs, .seconds, number: number)
        let wait = quantityText(maxWaitSecs, .seconds, number: number)
        return "A benchmark starts once \(loads) for \(quiet). If that takes longer than \(wait), "
            + "it starts anyway, marked not quiet."
    }

    /// "Keeps ended leases for 30 days and job runs for 90 days. Each command keeps at least
    /// its newest 20 runs, and never more than 5,000 runs are stored."
    public static func history(
        leaseDays: Double,
        historyDays: Double,
        keepRuns: Double,
        maxRuns: Double,
        number: (Double) -> String
    ) -> String {
        let newest = keepRuns == 1 ? "newest run" : "newest \(number(keepRuns)) runs"
        let most = maxRuns == 1 ? "1 run is" : "\(number(maxRuns)) runs are"
        let leases = quantityText(leaseDays, .days, number: number)
        let runs = quantityText(historyDays, .days, number: number)
        return "Keeps ended leases for \(leases) and job runs for \(runs). Each command keeps at "
            + "least its \(newest), and never more than \(most) stored."
    }

    private static func cores(_ cores: Double, _ number: (Double) -> String) -> String {
        quantityText(cores, .cores, number: number)
    }

    private static func gigabytes(_ megabytes: Double, _ number: (Double) -> String) -> String {
        number((megabytes / 1024 * 10).rounded(.toNearestOrAwayFromZero) / 10)
    }
}
