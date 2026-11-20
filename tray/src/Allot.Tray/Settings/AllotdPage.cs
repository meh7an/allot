using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// A page of allotd's settings, built from the spec once <c>GET /settings</c> first answers: the
/// page's groups as cards, each ending in its live sentence, then an Advanced section with the
/// rarely changed keys (DESIGN.md D11), then Reset page. Extras such as the Service status row go
/// in before, between or after the groups.
/// </summary>
internal sealed class AllotdPage
{
    private readonly PageContext _context;
    private readonly StackPanel _panel = new() { Padding = new Thickness(32, 16, 32, 32) };
    private readonly TextBlock _intro;
    private readonly List<Action> _refreshers = [];
    private bool _built;

    /// <param name="id">The spec's page id.</param>
    /// <param name="title">The page title, the same as its navigation item.</param>
    /// <param name="context">The session and what every editor shares.</param>
    public AllotdPage(string id, string title, PageContext context)
    {
        Id = id;
        _context = context;
        var heading = SettingsUi.Text(title, "TitleTextBlockStyle");
        AutomationProperties.SetHeadingLevel(heading, AutomationHeadingLevel.Level1);
        _intro = SettingsUi.Text(string.Empty, "BodyTextBlockStyle");
        _intro.Margin = new Thickness(0, 4, 0, 0);
        _intro.Visibility = Visibility.Collapsed;
        _panel.Children.Add(heading);
        _panel.Children.Add(_intro);
        _panel.MaxWidth = 1000;
        _panel.HorizontalAlignment = HorizontalAlignment.Stretch;
        Root = new ScrollViewer { Content = _panel };
        context.Session.PropertyChanged += (_, _) => Refresh();
    }

    public string Id { get; }

    public ScrollViewer Root { get; }

    /// <summary>
    /// Cards shown above the groups, such as the Service status row; shown before allotd answers.
    /// </summary>
    public List<UIElement> Top { get; } = [];

    /// <summary>
    /// Cards after a group, by group id, such as What fits after Learn from past runs.
    /// </summary>
    public Dictionary<string, List<UIElement>> After { get; } = new(StringComparer.Ordinal);

    /// <summary>
    /// Cards after the groups and before Advanced, such as Files.
    /// </summary>
    public List<UIElement> End { get; } = [];

    /// <summary>
    /// The live sentence of a group, by group id; empty hides it.
    /// </summary>
    public Dictionary<string, Func<string>> Live { get; } = new(StringComparer.Ordinal);

    /// <summary>
    /// Whether the Reset group also offers Reset all service settings.
    /// </summary>
    public bool OffersResetAll { get; init; }

    /// <summary>
    /// Shows the cards that need no settings; call once the extras are in.
    /// </summary>
    public void ShowTop()
    {
        foreach (var element in Top)
        {
            _panel.Children.Add(element);
        }

        Refresh();
    }

    /// <summary>
    /// Builds the groups the first time the session has pages, and updates live sentences and the
    /// reset buttons.
    /// </summary>
    public void Refresh()
    {
        var page = _context.Session.Pages.FirstOrDefault(candidate => candidate.Spec.Id == Id);
        if (!_built && page is not null)
        {
            _built = true;
            Build(page);
        }

        foreach (var refresh in _refreshers)
        {
            refresh();
        }
    }

    private void Build(PageModel page)
    {
        if (page.Spec.Intro is { Length: > 0 } intro)
        {
            _intro.Text = intro;
            _intro.Visibility = Visibility.Visible;
        }

        var advanced = new List<(GroupModel Group, List<ValueModel> Settings)>();
        foreach (var group in page.Groups)
        {
            var normal = group.Settings.Where(model => !model.Spec.Advanced).ToList();
            var rare = group.Settings.Where(model => model.Spec.Advanced).ToList();
            if (normal.Count > 0)
            {
                AddGroup(group.Spec.Label, normal, group.Spec.Id);
            }

            if (After.TryGetValue(group.Spec.Id, out var extras))
            {
                foreach (var element in extras)
                {
                    _panel.Children.Add(element);
                }
            }

            if (rare.Count > 0)
            {
                advanced.Add((group, rare));
            }
        }

        foreach (var element in End)
        {
            _panel.Children.Add(element);
        }

        if (advanced.Count > 0)
        {
            _panel.Children.Add(SettingsUi.Text("Advanced", "SettingsSectionHeadingStyle"));
            foreach (var (group, settings) in advanced)
            {
                var live = group.Settings.All(model => model.Spec.Advanced) ? group.Spec.Id : null;
                AddGroup(group.Spec.Label, settings, live);
            }
        }

        AddReset(page.Spec.Label);
    }

    // A heading and a card of rows; the live sentence of `liveGroup`, if any, ends the card.
    private void AddGroup(string label, List<ValueModel> settings, string? liveGroup)
    {
        _panel.Children.Add(SettingsUi.Heading(label));
        var rows = new List<UIElement>();
        foreach (var model in settings)
        {
            rows.AddRange(ValueEditors.For(model, _context));
        }

        var card = SettingsUi.Card(rows);
        if (liveGroup is not null && Live.TryGetValue(liveGroup, out var sentence))
        {
            var text = SettingsUi.LiveSentence();
            ((StackPanel)card.Child).Children.Add(text);
            _refreshers.Add(() => SettingsUi.SetLive(text, sentence()));
        }

        _panel.Children.Add(card);
    }

    private void AddReset(string pageLabel)
    {
        _panel.Children.Add(SettingsUi.Heading("Reset"));
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
        var resetPage = SettingsUi.Button("Reset page…", () => _ = ResetPageAsync(pageLabel));
        buttons.Children.Add(resetPage);
        Button? resetAll = null;
        if (OffersResetAll)
        {
            resetAll = SettingsUi.Button(
                "Reset all service settings…",
                () => _ = ResetAllAsync());
            buttons.Children.Add(resetAll);
        }

        var nothing = SettingsUi.Text(SettingsText.NothingToReset, "SettingsHelpTextStyle");
        nothing.Margin = new Thickness(0, 8, 0, 0);
        _panel.Children.Add(buttons);
        _panel.Children.Add(nothing);
        _refreshers.Add(() =>
        {
            var session = _context.Session;
            var page = !session.ReadOnly && session.ChangedOn(Id).Count > 0;
            var all = !session.ReadOnly && session.ChangedSettings().Count > 0;
            resetPage.IsEnabled = page;
            if (resetAll is not null)
            {
                resetAll.IsEnabled = all;
            }

            nothing.Visibility = SettingsUi.Shows(!page && !(OffersResetAll && all));
        });
    }

    private async Task ResetPageAsync(string pageLabel)
    {
        var changed = _context.Session.ChangedOn(Id);
        if (changed.Count == 0)
        {
            return;
        }

        var confirmation = SettingsText.ResetPage(pageLabel, changed, _context.Number);
        if (await _context.Confirm(confirmation).ConfigureAwait(true))
        {
            _context.Session.ResetChanged(Id);
        }
    }

    private async Task ResetAllAsync()
    {
        var changed = _context.Session.ChangedSettings();
        if (changed.Count == 0)
        {
            return;
        }

        if (await _context.Confirm(SettingsText.ResetAll(changed.Count)).ConfigureAwait(true))
        {
            _context.Session.ResetChanged(null);
        }
    }
}
