namespace Allot.Tray.Core.Settings;

/// <summary>
/// One switch on the Settings window's Panel page, over a toggle in <c>tray.json</c>.
/// </summary>
/// <param name="Label">The switch's label.</param>
/// <param name="Help">The line under it, or empty.</param>
/// <param name="Get">Reads the toggle.</param>
/// <param name="Set">Writes the toggle.</param>
/// <param name="Parent">The toggle this one is indented under and needs on, or <c>null</c>.</param>
public sealed record PanelToggle(
    string Label,
    string Help,
    Func<TraySettings, bool> Get,
    Action<TraySettings, bool> Set,
    PanelToggle? Parent = null)
{
    private static readonly TraySettings Fresh = new();

    /// <summary>
    /// The toggle's value in a fresh <c>tray.json</c>.
    /// </summary>
    public bool Default => Get(Fresh);

    /// <summary>
    /// Whether the switch can be changed: its parent, if any, is on.
    /// </summary>
    public bool EnabledIn(TraySettings settings) => Parent is null || Parent.Get(settings);
}

/// <summary>
/// The Panel page's Meters and Notifications groups (DESIGN.md section 4, Panel), and its Reset
/// page. Launch at logon lives in the autostart entry, not here, and Reset page leaves it alone.
/// </summary>
public static class PanelToggles
{
    public const string MetersHelp = "Rows shown in the flyout.";

    private static readonly PanelToggle Drives = new(
        "Drives",
        string.Empty,
        settings => settings.Meters.Drives,
        (settings, on) => settings.Meters.Drives = on);

    public static IReadOnlyList<PanelToggle> Meters { get; } =
    [
        new(
            "CPU",
            string.Empty,
            settings => settings.Meters.Cpu,
            (settings, on) => settings.Meters.Cpu = on),
        new(
            "RAM",
            string.Empty,
            settings => settings.Meters.Ram,
            (settings, on) => settings.Meters.Ram = on),
        new(
            "GPU",
            string.Empty,
            settings => settings.Meters.Gpu,
            (settings, on) => settings.Meters.Gpu = on),
        new(
            "VRAM",
            string.Empty,
            settings => settings.Meters.Vram,
            (settings, on) => settings.Meters.Vram = on),
        Drives,
        new(
            "Drive activity",
            string.Empty,
            settings => settings.Meters.DriveActivity,
            (settings, on) => settings.Meters.DriveActivity = on,
            Drives),
    ];

    public static IReadOnlyList<PanelToggle> Notifications { get; } =
    [
        new(
            "Service stopped",
            "When the service stops without you stopping it.",
            settings => settings.Notifications.ServiceStopped,
            (settings, on) => settings.Notifications.ServiceStopped = on),
        new(
            "Running long",
            "When a job runs 1.5 times longer than expected.",
            settings => settings.Notifications.RunningLong,
            (settings, on) => settings.Notifications.RunningLong = on),
        new(
            "Benchmark finished",
            "When a benchmark you started ends.",
            settings => settings.Notifications.BenchmarkFinished,
            (settings, on) => settings.Notifications.BenchmarkFinished = on),
        new(
            "Job finished",
            "When any job ends.",
            settings => settings.Notifications.JobFinished,
            (settings, on) => settings.Notifications.JobFinished = on),
        new(
            "Killed outside the flyout",
            "When a job is stopped by something other than this flyout.",
            settings => settings.Notifications.KilledOutsideFlyout,
            (settings, on) => settings.Notifications.KilledOutsideFlyout = on),
        new(
            "Update available",
            "When a newer version of Allot is released.",
            settings => settings.Notifications.UpdateAvailable,
            (settings, on) => settings.Notifications.UpdateAvailable = on),
    ];

    /// <summary>
    /// The toggles that differ from their default, for Reset page.
    /// </summary>
    public static IReadOnlyList<PanelToggle> Changed(TraySettings settings) =>
        [.. Meters.Concat(Notifications).Where(toggle => toggle.Get(settings) != toggle.Default)];

    /// <summary>
    /// Puts every meter and notification toggle back to its default.
    /// </summary>
    public static void Reset(TraySettings settings)
    {
        foreach (var toggle in Meters.Concat(Notifications))
        {
            toggle.Set(settings, toggle.Default);
        }
    }
}
