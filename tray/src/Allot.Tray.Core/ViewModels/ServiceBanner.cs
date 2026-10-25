using System.Globalization;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.Daemon;

namespace Allot.Tray.Core.ViewModels;

public enum BannerButton
{
    /// <summary>
    /// No button: starting allotd again would not help.
    /// </summary>
    None,

    /// <summary>
    /// The accent Start service button.
    /// </summary>
    Start,

    /// <summary>
    /// The muted, disabled Starting… that follows a press.
    /// </summary>
    Starting,
}

/// <summary>
/// The banner at the top of the body while the service is down, and the caption above the last
/// known cards (handoff section 6).
/// </summary>
/// <param name="Text">The semibold banner line.</param>
/// <param name="Problem">Why the last Start service failed, or empty.</param>
/// <param name="Button">The button beside the text.</param>
/// <param name="Caption">For example <c>Last known · 6:52 AM</c>; empty without a last known
/// state, which also means no cards.</param>
public sealed record ServiceBanner(string Text, string Problem, BannerButton Button, string Caption)
{
    public const string StoppedText = "Service stopped · commands run unchecked";

    public const string IncompatibleText = "Service needs an update · run allot install";

    /// <summary>
    /// No banner, while the service runs. The flyout model holds this instead of <c>null</c>:
    /// x:Bind skips a function binding whose argument path ends in <c>null</c>, so a banner set
    /// to <c>null</c> would stay on screen.
    /// </summary>
    public static ServiceBanner Hidden { get; } = new("", "", BannerButton.None, "");

    /// <param name="status">The service status of the latest poll.</param>
    /// <param name="lastKnown">The last state allotd served, if any.</param>
    /// <param name="startProblem">Why the last Start service failed, if it did.</param>
    /// <param name="zone">The time zone the caption's time is shown in.</param>
    /// <param name="culture">The culture that formats the caption's time.</param>
    /// <returns><c>null</c> while the service runs.</returns>
    public static ServiceBanner? From(
        ServiceStatus status,
        UiState? lastKnown,
        string? startProblem,
        TimeZoneInfo zone,
        CultureInfo culture)
    {
        var caption = lastKnown is null
            ? ""
            : $"Last known · {LocalTime(lastKnown.GeneratedAtMs, zone, culture)}";
        return status switch
        {
            ServiceStatus.Running => null,
            ServiceStatus.Starting =>
                new ServiceBanner(StoppedText, "", BannerButton.Starting, caption),
            ServiceStatus.Incompatible =>
                new ServiceBanner(IncompatibleText, "", BannerButton.None, caption),
            _ => new ServiceBanner(StoppedText, startProblem ?? "", BannerButton.Start, caption),
        };
    }

    /// <summary>
    /// A daemon time as the user's short time of day, for example <c>6:52 AM</c>.
    /// </summary>
    public static string LocalTime(long ms, TimeZoneInfo zone, CultureInfo culture) =>
        TimeZoneInfo.ConvertTime(DateTimeOffset.FromUnixTimeMilliseconds(ms), zone)
            .ToString("t", culture);
}
