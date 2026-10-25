using Allot.Tray.Core.Settings;

namespace Allot.Tray.Core.ViewModels;

public static class FirstRun
{
    /// <summary>
    /// The first-run card shows for the first time ever: the flyout opens by itself and Launch at
    /// logon takes its default, on. Later launches leave both alone, even before Done.
    /// </summary>
    public static bool ShowsForTheFirstTime(FlyoutSnapshot snapshot, TraySettings settings) =>
        snapshot.Welcome is not null && !settings.FirstRunShown;
}
