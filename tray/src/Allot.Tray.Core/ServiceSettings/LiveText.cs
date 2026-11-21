using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The live sentence at the end of each group with numbers (DESIGN.md D10), built from the values
/// on screen or from allotd's fit preview.
/// </summary>
public static class LiveText
{
    public const string ServiceStopped = "Can't work this out while the service is stopped.";

    public const string LoweringOff =
        "Requests are reserved as declared, or higher when past runs used more.";

    public const string NoneFit = "None fit";

    // The example in the learned-estimate sentence: a command asking for 8 cores that used 3.
    private const double ExampleAsked = 8;
    private const double ExampleUsed = 3;

    /// <summary>
    /// "Keeps 2.4 of 16 cores, 4.8 of 32 GB memory, 15 % of the GPU and 1.2 of 8 GB video memory
    /// free." The GPU parts are left out without a dedicated adapter.
    /// </summary>
    public static string Headroom(KeptFree kept, Func<double, string> number)
    {
        var parts = new List<string>
        {
            $"{number(kept.Cores)} of {number(kept.TotalCores)} cores",
            $"{Gigabytes(kept.RamMb, number)} of {Gigabytes(kept.TotalRamMb, number)} GB memory",
        };
        if (kept.GpuPct is { } gpu)
        {
            parts.Add($"{number(gpu)} % of the GPU");
        }

        if (kept.VramMb is { } vram && kept.TotalVramMb is { } totalVram)
        {
            parts.Add(
                $"{Gigabytes(vram, number)} of {Gigabytes(totalVram, number)} GB video memory");
        }

        return $"Keeps {SettingsText.Joined(parts)} free.";
    }

    /// <summary>
    /// What learning from past runs does to an example command, with the values on screen.
    /// </summary>
    /// <param name="lower">Whether lowering is on.</param>
    /// <param name="minRuns">Runs before lowering.</param>
    /// <param name="marginPct">The safety margin.</param>
    /// <param name="number">Formats a number for the user's locale.</param>
    public static string Estimates(
        bool lower,
        double minRuns,
        double marginPct,
        Func<double, string> number)
    {
        if (!lower)
        {
            return LoweringOff;
        }

        var reserved = Math.Round(
            ExampleUsed * (1 + (marginPct / 100)), 2, MidpointRounding.AwayFromZero);
        var runs = minRuns == 1 ? "run" : $"{number(minRuns)} runs";
        return $"A command that asks for {number(ExampleAsked)} cores but used about "
            + $"{number(ExampleUsed)} in its last {runs} is reserved {Cores(reserved, number)}. "
            + "Requests never go below 0.25 cores, 256 MB, or their minimum cores, and run time "
            + "never lowers.";
    }

    /// <summary>
    /// A cell of What fits: "1 at full size, 2 more at 4 cores", "at least 99 at full size", or
    /// "None fit".
    /// </summary>
    public static string Fits(Fits fits, double minCores, Func<double, string> number)
    {
        if (fits.Full == 0 && fits.Shrunk == 0)
        {
            return NoneFit;
        }

        // Counts stop at allotd's limit; the larger count then reads "at least 99".
        var capped = fits.More ? "at least " : string.Empty;
        var parts = new List<string>();
        if (fits.Full > 0)
        {
            var full = fits.Shrunk == 0 ? capped : string.Empty;
            parts.Add($"{full}{number(fits.Full)} at full size");
        }

        if (fits.Shrunk > 0)
        {
            var more = fits.Full > 0 ? " more" : string.Empty;
            parts.Add($"{capped}{number(fits.Shrunk)}{more} at {Cores(minCores, number)}");
        }

        return string.Join(", ", parts);
    }

    /// <summary>
    /// The caption under What fits.
    /// </summary>
    public static string FitsCaption(double cpuBusyPct, Func<double, string> number)
    {
        // Half away from zero, as the other panels round: 12.5 reads 13.
        var busy = Math.Round(cpuBusyPct, MidpointRounding.AwayFromZero);
        return $"Based on this machine now ({number(busy)} % CPU busy) and with every lease ended.";
    }

    /// <summary>
    /// "A benchmark starts once CPU and GPU stay at or below 10 % for 10 seconds. If that takes
    /// longer than 1 minute, it starts anyway, marked not quiet."
    /// </summary>
    public static string Benchmarks(
        double cpuPct,
        double gpuPct,
        double quietSecs,
        double maxWaitSecs,
        Func<double, string> number)
    {
        var loads = cpuPct == gpuPct
            ? $"CPU and GPU stay at or below {Percent(cpuPct, number)}"
            : $"CPU stays at or below {Percent(cpuPct, number)} and GPU at or below "
                + Percent(gpuPct, number);
        return $"A benchmark starts once {loads} for {Duration(quietSecs, number)}. If that takes "
            + $"longer than {Duration(maxWaitSecs, number)}, it starts anyway, marked not quiet.";
    }

    /// <summary>
    /// "Keeps ended leases for 30 days and job runs for 90 days. Each command keeps at least its
    /// newest 20 runs, and never more than 5,000 runs are stored."
    /// </summary>
    public static string History(
        double leaseDays,
        double historyDays,
        double keepRuns,
        double maxRuns,
        Func<double, string> number)
    {
        var newest = keepRuns == 1 ? "newest run" : $"newest {number(keepRuns)} runs";
        var most = maxRuns == 1 ? "1 run is" : $"{number(maxRuns)} runs are";
        return $"Keeps ended leases for {Days(leaseDays, number)} and job runs for "
            + $"{Days(historyDays, number)}. Each command keeps at least its {newest}, and never "
            + $"more than {most} stored.";
    }

    private static string Percent(double value, Func<double, string> number) =>
        ValueText.QuantityText(value, Quantity.Percent, number);

    private static string Duration(double seconds, Func<double, string> number) =>
        ValueText.QuantityText(seconds, Quantity.Seconds, number);

    private static string Days(double days, Func<double, string> number) =>
        ValueText.QuantityText(days, Quantity.Days, number);

    private static string Cores(double cores, Func<double, string> number) =>
        ValueText.QuantityText(cores, Quantity.Cores, number);

    // Rounds half away from zero: 256 MB reads 0.3 GB, as in the other panels.
    private static string Gigabytes(double megabytes, Func<double, string> number) =>
        number(Math.Round(megabytes / 1024, 1, MidpointRounding.AwayFromZero));
}
