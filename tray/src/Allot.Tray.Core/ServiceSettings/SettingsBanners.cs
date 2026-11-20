using System.Globalization;

namespace Allot.Tray.Core.ServiceSettings;

public enum BannerSeverity
{
    Informational,
    Warning,
    Error,
}

/// <summary>
/// The buttons a banner can carry.
/// </summary>
public enum BannerAction
{
    StartService,
    RestartService,
    TryNow,
    OpenLogs,
    OpenFile,
    CheckAgain,
    ApplyMine,
    KeepFileValues,
    SaveAgain,
}

/// <summary>
/// A lasting state at the top of a Settings page (DESIGN.md section 7).
/// </summary>
/// <param name="Severity">How the banner looks.</param>
/// <param name="Message">What happened and what it means.</param>
/// <param name="Actions">Its buttons, the first one primary.</param>
public sealed record SettingsBanner(
    BannerSeverity Severity,
    string Message,
    IReadOnlyList<BannerAction> Actions);

/// <summary>
/// Which banners a page shows for the session's state. The Panel page shows none of allotd's: it
/// keeps working in every state.
/// </summary>
public static class SettingsBanners
{
    public const string PanelPage = "panel";

    public const string ServicePage = "service";

    private const string PortKey = "daemon.port";

    /// <param name="session">The settings session.</param>
    /// <param name="pageId">The page shown.</param>
    /// <param name="port">The port this panel reaches allotd on.</param>
    /// <param name="number">Formats a number for the user's locale.</param>
    public static IReadOnlyList<SettingsBanner> For(
        SettingsSession session,
        string pageId,
        uint port,
        Func<double, string> number)
    {
        if (pageId == PanelPage)
        {
            return [];
        }

        if (Unanswered(session, port) is { } unanswered)
        {
            return [unanswered];
        }

        var banners = new List<SettingsBanner>();
        if (session.Status != SessionStatus.Ready)
        {
            return banners;
        }

        if (session.FileError is { } fileError)
        {
            banners.Add(new SettingsBanner(
                BannerSeverity.Error,
                $"config.toml has an error: {Sentence(fileError)} The service keeps using its last "
                    + "good settings. Fix the file to change settings here.",
                [BannerAction.OpenFile, BannerAction.CheckAgain]));
        }

        if (session.Conflict is { } conflict)
        {
            banners.Add(new SettingsBanner(
                BannerSeverity.Warning,
                $"config.toml changed outside Settings. Showing the file's values. "
                    + NotSaved(conflict, number),
                [BannerAction.ApplyMine, BannerAction.KeepFileValues]));
        }

        if (session.WriteProblem is { } problem)
        {
            banners.Add(new SettingsBanner(
                BannerSeverity.Warning,
                $"Your changes are not saved yet. {Sentence(problem)}",
                [BannerAction.SaveAgain]));
        }

        if (pageId == ServicePage
            && session.PendingRestart.Contains(PortKey)
            && (session.Setting(PortKey) as NumberModel)?.Saved is { } newPort)
        {
            banners.Add(new SettingsBanner(
                BannerSeverity.Informational,
                $"Restart the service to use port {Port(newPort)}.",
                [BannerAction.RestartService]));
        }

        return banners;
    }

    /// <summary>
    /// The line under Port while a new port waits for a restart, or <c>null</c>.
    /// </summary>
    public static string? RestartNote(SettingsSession session, string key) =>
        session.PendingRestart.Contains(key) && session.ListeningPort is { } listening
            ? $"Restart needed. The service still listens on {Port(listening)}."
            : null;

    public static string Label(BannerAction action) => action switch
    {
        BannerAction.StartService => "Start service",
        BannerAction.RestartService => "Restart service",
        BannerAction.TryNow => "Try now",
        BannerAction.OpenLogs => "Open logs",
        BannerAction.OpenFile => "Open file",
        BannerAction.CheckAgain => "Check again",
        BannerAction.ApplyMine => "Apply my change",
        BannerAction.KeepFileValues => "Keep file's values",
        BannerAction.SaveAgain => "Try again",
        _ => throw new ArgumentOutOfRangeException(nameof(action), action, null),
    };

    private static SettingsBanner? Unanswered(SettingsSession session, uint port) =>
        session.Status switch
        {
            SessionStatus.Stopped => new SettingsBanner(
                BannerSeverity.Informational,
                "The service is stopped. Start it to change these settings.",
                [BannerAction.StartService]),
            SessionStatus.Down => new SettingsBanner(
                BannerSeverity.Warning,
                $"The service isn't answering on port {Port(port)}. Trying again every 5 s.",
                [BannerAction.TryNow, BannerAction.OpenLogs]),
            SessionStatus.Token => new SettingsBanner(
                BannerSeverity.Error,
                "The service refused this panel's key. Restart the service to make a new one.",
                [BannerAction.RestartService]),
            SessionStatus.Failed => new SettingsBanner(
                BannerSeverity.Error,
                $"The service sent an answer Settings can't use. {Sentence(session.Detail)}",
                [BannerAction.TryNow, BannerAction.OpenLogs]),
            _ => null,
        };

    // "Your change to Safety margin (15 %) was not saved."
    private static string NotSaved(Conflict conflict, Func<double, string> number)
    {
        var edits = conflict.Edits.Select(edit => Edit(edit, number)).ToList();
        return edits.Count == 1
            ? $"Your change to {edits[0]} was not saved."
            : $"Your changes to {SettingsText.Joined(edits)} were not saved.";
    }

    private static string Edit(ConflictEdit edit, Func<double, string> number)
    {
        if (edit.Changes.Count == 0)
        {
            return edit.Setting.Label;
        }

        var value = SettingsText.ChangeValue(edit.Setting, edit.Changes[0], number);
        return $"{edit.Setting.Label} ({value})";
    }

    // allotd's reasons come without a full stop; the banner's next sentence needs one.
    private static string Sentence(string? text)
    {
        var trimmed = (text ?? string.Empty).Trim();
        return trimmed.Length == 0 || trimmed.EndsWith('.') ? trimmed : trimmed + ".";
    }

    private static string Port(double port) => port.ToString(CultureInfo.InvariantCulture);
}
