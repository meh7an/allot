using System.Globalization;

namespace Allot.Tray.Core.TrayIcon;

/// <summary>
/// The states of the Allot mark in the tray: dimmed while idle, solid with a green play badge
/// while busy, a purple stopwatch badge during a benchmark, an amber clock badge while queued,
/// and dimmed with a red cross badge while the service is down. The badges follow the flyout's
/// running, benchmark and waiting icons.
/// </summary>
public enum TrayIconState
{
    Idle,
    Busy,
    Benchmark,
    Queued,
    Down,
}

/// <summary>
/// The taskbar's theme, which can differ from the app theme; the icon follows the taskbar.
/// </summary>
public enum TaskbarTheme
{
    Light,
    Dark,
}

/// <summary>
/// Which ICO file and tooltip the tray icon shows. assets/brand/build.py generates the files.
/// </summary>
public static class TrayIconAsset
{
    /// <summary>
    /// NOTIFYICONDATAW.szTip holds 128 characters including the terminator.
    /// </summary>
    public const int MaxTooltipLength = 127;

    private const string AppName = "Allot";

    public static string FileName(TrayIconState state, TaskbarTheme theme) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"allot-{StateName(state)}-on-{ThemeName(theme)}-taskbar.ico");

    /// <summary>
    /// The 256 px PNG of the same icon, a toast's logo: toast images cannot be ICO files. Toasts
    /// follow the Windows mode, which is the taskbar theme.
    /// </summary>
    public static string LogoFileName(TrayIconState state, TaskbarTheme theme) =>
        string.Create(
            CultureInfo.InvariantCulture,
            $"allot-{StateName(state)}-on-{ThemeName(theme)}-taskbar-256.png");

    /// <summary>
    /// Reads HKCU\...\Themes\Personalize\SystemUsesLightTheme. The value is missing only on
    /// Windows builds whose taskbar is always dark.
    /// </summary>
    public static TaskbarTheme ThemeFrom(int? systemUsesLightTheme) =>
        systemUsesLightTheme is null or 0 ? TaskbarTheme.Dark : TaskbarTheme.Light;

    public static string Tooltip(string summary)
    {
        var text = $"{AppName} · {summary}";
        return text.Length <= MaxTooltipLength
            ? text
            : string.Concat(text.AsSpan(0, MaxTooltipLength - 1), "…");
    }

    private static string StateName(TrayIconState state) => state switch
    {
        TrayIconState.Idle => "idle",
        TrayIconState.Busy => "busy",
        TrayIconState.Benchmark => "benchmark",
        TrayIconState.Queued => "queued",
        TrayIconState.Down => "down",
        _ => throw new ArgumentOutOfRangeException(nameof(state)),
    };

    private static string ThemeName(TaskbarTheme theme) => theme switch
    {
        TaskbarTheme.Light => "light",
        TaskbarTheme.Dark => "dark",
        _ => throw new ArgumentOutOfRangeException(nameof(theme)),
    };
}
