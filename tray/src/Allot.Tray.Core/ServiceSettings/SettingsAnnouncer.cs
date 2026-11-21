namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// A setting with an error, with the sentence to speak.
/// </summary>
/// <param name="Key">The setting's key.</param>
/// <param name="Text">"Headroom: Enter a number at least 0 and below 100."</param>
public sealed record AnnouncedIssue(string Key, string Text);

/// <summary>
/// What the announcer compares between two changes of the window.
/// </summary>
/// <param name="Banners">The shown page's banner messages.</param>
/// <param name="Issues">Each setting with an error.</param>
/// <param name="Save">The save status's kind.</param>
public sealed record AnnouncerView(
    IReadOnlyList<string> Banners,
    IReadOnlyList<AnnouncedIssue> Issues,
    SaveKind Save)
{
    /// <summary>
    /// The announcer's view of a session and the banners of the page shown.
    /// </summary>
    public static AnnouncerView Of(
        SettingsSession session,
        IReadOnlyList<SettingsBanner> banners,
        Func<double, string> number)
    {
        var issues = new List<AnnouncedIssue>();
        foreach (var page in session.Pages)
        {
            foreach (var group in page.Groups)
            {
                foreach (var setting in group.Settings)
                {
                    if (setting.Issue is { } issue)
                    {
                        issues.Add(new AnnouncedIssue(
                            setting.Key,
                            $"{setting.Label}: {ValueText.IssueText(issue, number)}"));
                    }
                }
            }
        }

        return new AnnouncerView(
            [.. banners.Select(banner => banner.Message)],
            issues,
            session.SaveStatus.Kind);
    }
}

/// <summary>
/// What a Settings window says to a screen reader by itself, without a focus change (Mehran at
/// M7.1: errors and outcomes): each new or changed error once, each banner as it appears, and
/// "All changes saved" when a save ends with nothing left unsaved. Live sentences and What fits
/// are read with their group when focus reaches it. The first view, and the first after another
/// page shows, only sets what is already on screen.
/// </summary>
public sealed class SettingsAnnouncer
{
    public const string SavedAnnouncement = "All changes saved";

    private HashSet<string>? _banners;
    private Dictionary<string, string> _issues = [];
    private bool _saving;

    /// <summary>
    /// What to speak for this view, as one text; empty when nothing spoken changed.
    /// </summary>
    public string Update(AnnouncerView view)
    {
        var spoken = new List<string>();
        if (_banners is { } shown)
        {
            spoken.AddRange(view.Banners.Where(message => !shown.Contains(message)));
        }

        foreach (var issue in view.Issues)
        {
            if (!_issues.TryGetValue(issue.Key, out var before) || before != issue.Text)
            {
                spoken.Add(issue.Text);
            }
        }

        if (_saving && view.Save == SaveKind.Saved && spoken.Count == 0)
        {
            spoken.Add(SavedAnnouncement);
        }

        _banners = [.. view.Banners];
        _issues = view.Issues.ToDictionary(issue => issue.Key, issue => issue.Text);
        _saving = view.Save == SaveKind.Saving;
        return string.Join(' ', spoken);
    }

    /// <summary>
    /// Another page shows: its banners are read with the page, not announced.
    /// </summary>
    public void PageShown() => _banners = null;
}
