using Allot.Tray.Core.Settings;

namespace Allot.Tray.Core.ViewModels;

/// <summary>
/// The trailing counts of the ⋯ menu's Meters and Notifications items.
/// </summary>
public static class MenuCounts
{
    private const int MeterItems = 5;
    private const int NotificationItems = 5;

    /// <summary>
    /// For example <c>4 of 5</c>; Drive activity is part of Drives, so it does not count.
    /// </summary>
    public static string Meters(MeterToggles toggles)
    {
        bool[] on = [toggles.Cpu, toggles.Ram, toggles.Gpu, toggles.Vram, toggles.Drives];
        return $"{on.Count(item => item)} of {MeterItems}";
    }

    public static string Notifications(NotificationToggles toggles)
    {
        bool[] on =
        [
            toggles.ServiceStopped,
            toggles.RunningLong,
            toggles.BenchmarkFinished,
            toggles.JobFinished,
            toggles.KilledOutsideFlyout,
        ];
        return $"{on.Count(item => item)} of {NotificationItems}";
    }
}
