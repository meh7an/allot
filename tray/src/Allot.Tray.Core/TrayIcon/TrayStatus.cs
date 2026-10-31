using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.TrayIcon;

/// <summary>
/// What the tray icon shows for one snapshot: the mark's state and the summary that the
/// tooltip and the menu's status line repeat.
/// </summary>
/// <param name="Icon">The icon state.</param>
/// <param name="Summary">The header summary.</param>
public sealed record TrayStatus(TrayIconState Icon, string Summary)
{
    public string Tooltip => TrayIconAsset.Tooltip(Summary);

    /// <summary>
    /// Down while allotd is not serving state; Benchmark while a benchmark drains, waits for quiet
    /// or runs alone; Queued while anything else waits, since its icon also shows bars; Busy while
    /// a job runs; Idle otherwise.
    /// </summary>
    public static TrayStatus From(FlyoutSnapshot snapshot)
    {
        var header = snapshot.Header;
        var icon = snapshot.IsLastKnown ? TrayIconState.Down
            : snapshot.Benchmark is not null ? TrayIconState.Benchmark
            : IsWaiting(snapshot) ? TrayIconState.Queued
            : header.Glyph == HeaderGlyph.Busy ? TrayIconState.Busy
            : TrayIconState.Idle;
        return new TrayStatus(icon, header.Summary);
    }

    private static bool IsWaiting(FlyoutSnapshot snapshot) =>
        snapshot.Groups.SelectMany(group => group.Jobs)
            .Any(job => job.Queued is not null && !job.Ending);
}
