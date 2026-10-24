using System.Globalization;

using ResourcesHelper.Tray.Core.Contract;

namespace ResourcesHelper.Tray.Core.ViewModels;

/// <summary>
/// Where the Drain / Quiet / Run stepper stands.
/// </summary>
public enum BenchmarkStep
{
    Drain,
    Quiet,
    Run,

    /// <summary>
    /// The run finished; every step is complete.
    /// </summary>
    Done,
}

/// <summary>
/// The leading icon of a card's detail line.
/// </summary>
public enum DetailIcon
{
    /// <summary>
    /// No icon; the line truncates with an ellipsis.
    /// </summary>
    None,

    /// <summary>
    /// The purple timer of a benchmark in progress; the line wraps.
    /// </summary>
    Timer,

    /// <summary>
    /// The green check of a finished benchmark; the line wraps.
    /// </summary>
    Check,
}

/// <summary>
/// The stepper under a benchmark's status line.
/// </summary>
/// <param name="Current">The step in progress, or <see cref="BenchmarkStep.Done"/>.</param>
/// <param name="QuietTicks">Filled ticks of the Quiet step, out of <see cref="TickCount"/>.</param>
/// <param name="RunFraction">How much of the Run step is filled, from 0 to 1.</param>
public sealed record BenchmarkSteps(BenchmarkStep Current, int QuietTicks, double RunFraction)
{
    /// <summary>
    /// Ticks of the Quiet step, one per second of a 10 s quiet window.
    /// </summary>
    public const int TickCount = 10;
}

/// <summary>
/// A benchmark card's status line and stepper (handoff section 4, Benchmark).
/// </summary>
/// <param name="Text">The status line.</param>
/// <param name="Icon">Its leading icon.</param>
/// <param name="Steps">The stepper.</param>
public sealed record BenchmarkStatus(string Text, DetailIcon Icon, BenchmarkSteps Steps)
{
    private const string NotQuiet = "not quiet at start · ";

    /// <param name="job">The benchmark, granted and running alone.</param>
    /// <param name="elapsed">Seconds since it was granted.</param>
    public static BenchmarkStatus ForActive(ActiveJob job, long elapsed)
    {
        var estimate = (long)job.Lease.Profile.EstSecs;
        var over = estimate > 0 && elapsed > estimate;
        var time = over ? $"{Format.Duration(elapsed - estimate)} over"
            : estimate > 0 ? $"{Format.Duration(elapsed)} / ~{Format.Duration(estimate)}"
            : Format.Duration(elapsed);
        var forced = job.QuietAtStart == false ? NotQuiet : "";
        var fraction = estimate > 0 ? Math.Clamp((double)elapsed / estimate, 0, 1) : 0;
        return new BenchmarkStatus(
            $"Running alone · {forced}{time}",
            DetailIcon.Timer,
            new BenchmarkSteps(BenchmarkStep.Run, BenchmarkSteps.TickCount, fraction));
    }

    /// <param name="job">The benchmark, still queued. Only the head of the queue carries
    /// progress.</param>
    /// <param name="othersActive">Another lease is still active, so a benchmark without
    /// progress waits for it to drain; otherwise it waits for room, with rhd's first
    /// reason.</param>
    public static BenchmarkStatus ForQueued(QueuedJob job, bool othersActive) =>
        job.Verdict.Benchmark switch
        {
            BenchmarkProgress.Measuring measuring => new BenchmarkStatus(
                string.Create(
                    CultureInfo.InvariantCulture,
                    $"Waiting for quiet · {measuring.MeasuredSecs} / {measuring.NeededSecs} s"
                        + $" · background {measuring.CpuAvgPct:0}% CPU"),
                DetailIcon.Timer,
                new BenchmarkSteps(BenchmarkStep.Quiet, Ticks(measuring), 0)),
            // The window is full but too busy; rhd grants it anyway once the longest wait ends.
            BenchmarkProgress.Noisy noisy => new BenchmarkStatus(
                string.Create(
                    CultureInfo.InvariantCulture,
                    $"Not quiet · background {noisy.CpuAvgPct:0}% CPU"
                        + $" (limit {noisy.CpuLimitPct:0}%)"
                        + $" · starting anyway {In(noisy.GrantAnywayInSecs)}"),
                DetailIcon.Timer,
                new BenchmarkSteps(BenchmarkStep.Quiet, BenchmarkSteps.TickCount, 0)),
            BenchmarkProgress.Draining => Draining,
            _ when othersActive => Draining,
            // Drained, but its own reservation does not fit (seen live: 16 cores, 12.4 free).
            _ => new BenchmarkStatus(
                job.Verdict.Reasons.Count == 0
                    ? "Waiting for room"
                    : $"Waiting for room · {job.Verdict.Reasons[0]}",
                DetailIcon.Timer,
                new BenchmarkSteps(BenchmarkStep.Drain, 0, 0)),
        };

    private static BenchmarkStatus Draining { get; } = new(
        "Draining · waiting for running jobs to finish",
        DetailIcon.Timer,
        new BenchmarkSteps(BenchmarkStep.Drain, 0, 0));

    /// <param name="run">The finished benchmark run.</param>
    /// <param name="noise">Its background load.</param>
    public static BenchmarkStatus ForFinished(RecentRun run, NoiseReport noise)
    {
        var ran = Math.Max(0, (run.EndedAtMs - run.StartedAtMs) / 1000);
        var forced = noise.QuietAtStart ? "" : NotQuiet;
        return new BenchmarkStatus(
            string.Create(
                CultureInfo.InvariantCulture,
                $"Finished in {Format.Duration(ran)} · {forced}background noise avg "
                    + $"{noise.AvgCpuPct:0.0}%, peak {noise.PeakCpuPct:0.0}% CPU"),
            DetailIcon.Check,
            new BenchmarkSteps(BenchmarkStep.Done, BenchmarkSteps.TickCount, 1));
    }

    // The window may be configured longer or shorter than 10 s; the ticks show its share.
    private static int Ticks(BenchmarkProgress.Measuring measuring) =>
        measuring.NeededSecs == 0
            ? BenchmarkSteps.TickCount
            : (int)Math.Min(
                BenchmarkSteps.TickCount,
                measuring.MeasuredSecs * BenchmarkSteps.TickCount / measuring.NeededSecs);

    private static string In(ulong seconds) =>
        seconds == 0 ? "now" : $"in {Format.Duration((long)seconds)}";
}
