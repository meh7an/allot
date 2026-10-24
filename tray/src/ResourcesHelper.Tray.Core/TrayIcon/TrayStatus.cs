using ResourcesHelper.Tray.Core.ViewModels;

namespace ResourcesHelper.Tray.Core.TrayIcon;

/// <summary>
/// What the tray icon shows for one snapshot: the meter-mark state and the summary that the
/// tooltip and the menu's status line repeat.
/// </summary>
/// <param name="Icon">The icon state.</param>
/// <param name="Summary">The header summary.</param>
public sealed record TrayStatus(TrayIconState Icon, string Summary)
{
    public string Tooltip => TrayIconAsset.Tooltip(Summary);

    /// <summary>
    /// Down while rhd is not serving state; Queued while anything waits, since its icon also
    /// shows bars; Busy while a job runs or a benchmark runs alone; Idle otherwise.
    /// </summary>
    public static TrayStatus From(FlyoutSnapshot snapshot)
    {
        var header = snapshot.Header;
        var icon = snapshot.IsLastKnown ? TrayIconState.Down
            : IsWaiting(snapshot) ? TrayIconState.Queued
            : header.Glyph == HeaderGlyph.Busy ? TrayIconState.Busy
            : TrayIconState.Idle;
        return new TrayStatus(icon, header.Summary);
    }

    private static bool IsWaiting(FlyoutSnapshot snapshot) =>
        snapshot.Groups.SelectMany(group => group.Jobs)
            .Any(job => job.Queued is not null && !job.Ending);
}
