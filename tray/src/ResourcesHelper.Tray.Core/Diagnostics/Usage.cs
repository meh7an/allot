using System.Globalization;

namespace ResourcesHelper.Tray.Core.Diagnostics;

/// <summary>
/// Process totals at one moment; two readings give the usage in between.
/// </summary>
/// <param name="Wall">Time since a fixed origin.</param>
/// <param name="Cpu">CPU time the process used since it started.</param>
/// <param name="AllocatedBytes">Managed bytes allocated since it started.</param>
/// <param name="Gen0">Generation 0 collections so far.</param>
/// <param name="Gen1">Generation 1 collections so far.</param>
/// <param name="Gen2">Generation 2 collections so far.</param>
/// <param name="PrivateBytes">Private memory now.</param>
/// <param name="WorkingSetBytes">Working set now.</param>
public readonly record struct ProcessReading(
    TimeSpan Wall,
    TimeSpan Cpu,
    long AllocatedBytes,
    int Gen0,
    int Gen1,
    int Gen2,
    long PrivateBytes,
    long WorkingSetBytes);

/// <summary>
/// The flyout updates (one snapshot applied to the model) between two readings.
/// </summary>
public sealed class UpdateTally
{
    public int Count { get; private set; }

    public TimeSpan Total { get; private set; }

    public TimeSpan Max { get; private set; }

    public long AllocatedBytes { get; private set; }

    public void Add(TimeSpan elapsed, long allocatedBytes)
    {
        Count++;
        Total += elapsed;
        Max = elapsed > Max ? elapsed : Max;
        AllocatedBytes += allocatedBytes;
    }

    public void Clear()
    {
        Count = 0;
        Total = TimeSpan.Zero;
        Max = TimeSpan.Zero;
        AllocatedBytes = 0;
    }
}

public static class Usage
{
    private const double Mebibyte = 1024 * 1024;

    /// <summary>
    /// For example <c>usage closed: 60 s, cpu 0.21% of one core, private 63.1 MB, working set
    /// 88.0 MB, allocated 1.2 MB, gc 0/0/0, 12 updates, avg 0.8 ms, max 2.1 ms, 14.0 KB each,
    /// 3 changes, 0 resizes</c>.
    /// </summary>
    /// <param name="flyout">Whether the flyout was open, closed or both in between.</param>
    /// <param name="from">The earlier reading.</param>
    /// <param name="to">The later reading.</param>
    /// <param name="updates">The updates in between.</param>
    /// <param name="changes">Property and collection changes raised in between.</param>
    /// <param name="resizes">Window size changes in between.</param>
    public static string Line(
        string flyout,
        ProcessReading from,
        ProcessReading to,
        UpdateTally updates,
        long changes,
        int resizes)
    {
        var wall = to.Wall - from.Wall;
        var cpu = wall > TimeSpan.Zero ? (to.Cpu - from.Cpu) / wall * 100 : 0;
        var average = updates.Count > 0 ? updates.Total / updates.Count : TimeSpan.Zero;
        var each = updates.Count > 0 ? updates.AllocatedBytes / 1024.0 / updates.Count : 0;
        return string.Create(
            CultureInfo.InvariantCulture,
            $"usage {flyout}: {wall.TotalSeconds:0} s, cpu {cpu:0.00}% of one core, "
                + $"private {to.PrivateBytes / Mebibyte:0.0} MB, "
                + $"working set {to.WorkingSetBytes / Mebibyte:0.0} MB, "
                + $"allocated {(to.AllocatedBytes - from.AllocatedBytes) / Mebibyte:0.00} MB, "
                + $"gc {to.Gen0 - from.Gen0}/{to.Gen1 - from.Gen1}/{to.Gen2 - from.Gen2}, "
                + $"{updates.Count} updates, avg {average.TotalMilliseconds:0.00} ms, "
                + $"max {updates.Max.TotalMilliseconds:0.00} ms, {each:0.0} KB each, "
                + $"{changes} changes, {resizes} resizes");
    }
}
