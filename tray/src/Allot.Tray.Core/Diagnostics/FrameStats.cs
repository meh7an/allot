using System.Globalization;

namespace Allot.Tray.Core.Diagnostics;

/// <summary>
/// The frame intervals of one animation, as the measure log reports them. Drops are counted
/// against 60 fps, the frame budget, whatever the display's refresh rate; the median shows that
/// rate.
/// </summary>
/// <param name="Frames">The number of intervals.</param>
/// <param name="MedianMs">The median interval.</param>
/// <param name="P95Ms">The 95th percentile interval (nearest rank).</param>
/// <param name="MaxMs">The longest interval.</param>
/// <param name="Dropped">Frames missed at 60 fps: an interval of two periods missed one.</param>
public sealed record FrameStats(
    int Frames,
    double MedianMs,
    double P95Ms,
    double MaxMs,
    int Dropped)
{
    /// <summary>
    /// One frame at 60 fps.
    /// </summary>
    public const double PeriodMs = 1000.0 / 60;

    public static FrameStats From(IReadOnlyList<double> intervalsMs)
    {
        if (intervalsMs.Count == 0)
        {
            return new FrameStats(0, 0, 0, 0, 0);
        }

        var sorted = intervalsMs.Order().ToArray();
        var dropped = intervalsMs.Sum(interval =>
            Math.Max(0, (int)Math.Round(interval / PeriodMs) - 1));
        return new FrameStats(
            sorted.Length,
            Rank(sorted, 0.5),
            Rank(sorted, 0.95),
            sorted[^1],
            dropped);
    }

    /// <summary>
    /// For example <c>open-slide: 15 frames, median 16.7 ms, p95 16.9 ms, max 33.4 ms, 1 dropped,
    /// 0 resizes</c>.
    /// </summary>
    /// <param name="name">The animation.</param>
    /// <param name="resizes">Window size changes while it ran.</param>
    public string Line(string name, int resizes) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"{name}: {Frames} frames, median {MedianMs:0.0} ms, p95 {P95Ms:0.0} ms, "
                + $"max {MaxMs:0.0} ms, {Dropped} dropped, {resizes} resizes");

    private static double Rank(double[] sorted, double percentile)
    {
        var rank = (int)Math.Ceiling(percentile * sorted.Length);
        return sorted[Math.Clamp(rank - 1, 0, sorted.Length - 1)];
    }
}
