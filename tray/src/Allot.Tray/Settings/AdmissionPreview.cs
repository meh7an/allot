using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// The Admission page's live parts from allotd's fit preview: the headroom sentence and the What
/// fits on this machine table. They follow the values on screen, unsaved edits included, 600 ms
/// after the last change, and the machine's load with every refresh while the page shows.
/// </summary>
internal sealed class AdmissionPreview
{
    public const string FitsGroup = "What fits on this machine";

    private const string NotYet = "Can't work this out yet. Trying again in a moment.";

    private readonly PageContext _context;
    private readonly DispatcherQueueTimer _debounce;
    private readonly Grid _table = new() { ColumnSpacing = 24, RowSpacing = 8 };
    private readonly TextBlock _caption = SettingsUi.LiveSentence();
    private FitPreview? _latest;
    private int _request;
    private bool _active;

    public AdmissionPreview(PageContext context, DispatcherQueue dispatcher)
    {
        _context = context;
        _debounce = dispatcher.CreateTimer();
        _debounce.Interval = SettingsSession.SaveDelay;
        _debounce.IsRepeating = false;
        _debounce.Tick += (_, _) => _ = RequestAsync();
        context.Session.PropertyChanged += (_, _) =>
        {
            if (_active)
            {
                _debounce.Stop();
                _debounce.Start();
            }
        };

        _table.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        _table.ColumnDefinitions.Add(new ColumnDefinition { Width = SettingsUi.Star });
        _table.ColumnDefinitions.Add(new ColumnDefinition { Width = SettingsUi.Star });
        var panel = new StackPanel { Children = { _table, _caption } };
        Card = new StackPanel
        {
            Children =
            {
                SettingsUi.Heading(FitsGroup),
                new Border { Style = SettingsUi.Style("SettingsCardStyle"), Child = panel },
            },
        };
        Show();
    }

    /// <summary>
    /// Raised when a preview arrived, so the page's live sentences update.
    /// </summary>
    public event Action? Updated;

    public UIElement Card { get; }

    /// <summary>
    /// The headroom sentence for the values on screen, or why it is not known.
    /// </summary>
    public string HeadroomText()
    {
        if (Unavailable() is { } reason)
        {
            return reason;
        }

        return _latest is null
            ? string.Empty
            : LiveText.Headroom(_latest.KeptFree, _context.Number);
    }

    /// <summary>
    /// Starts or stops previewing: on while the Admission page shows in a visible window.
    /// </summary>
    public void SetActive(bool active)
    {
        _active = active;
        _debounce.Stop();
        if (active)
        {
            _ = RequestAsync();
        }
    }

    private string? Unavailable() => _context.Session.Status switch
    {
        SessionStatus.Ready or SessionStatus.Loading => null,
        _ => LiveText.ServiceStopped,
    };

    private async Task RequestAsync()
    {
        if (!_active || _context.Session.Status != SessionStatus.Ready)
        {
            Show();
            return;
        }

        var request = ++_request;
        var reply = await _context.Session.PreviewAsync().ConfigureAwait(true);
        if (request != _request)
        {
            return;
        }

        if (reply.Value is { } preview)
        {
            _latest = preview;
        }
        else
        {
            AppLog.Write($"settings: fit preview failed: {reply.Failure?.Message}");
        }

        Show();
        Updated?.Invoke();
    }

    private void Show()
    {
        _table.Children.Clear();
        _table.RowDefinitions.Clear();
        var reason = Unavailable() ?? (_latest is null ? NotYet : null);
        if (reason is not null || _latest is null)
        {
            SettingsUi.SetLive(_caption, reason ?? NotYet);
            return;
        }

        AddRow(0, "Class", "Now", "When idle", header: true);
        var row = 1;
        foreach (var fit in _latest.Classes)
        {
            AddRow(
                row++,
                ClassLabel(fit.Name),
                LiveText.Fits(fit.Now, fit.MinCores, _context.Number),
                LiveText.Fits(fit.Idle, fit.MinCores, _context.Number),
                header: false);
        }

        SettingsUi.SetLive(_caption, LiveText.FitsCaption(_latest.CpuBusyPct, _context.Number));
    }

    private void AddRow(int row, string name, string now, string idle, bool header)
    {
        _table.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        var style = header ? "SettingsHelpTextStyle" : "BodyTextBlockStyle";
        string[] cells = [name, now, idle];
        for (var column = 0; column < cells.Length; column++)
        {
            var text = SettingsUi.Text(cells[column], style);
            Grid.SetRow(text, row);
            Grid.SetColumn(text, column);
            _table.Children.Add(text);
        }
    }

    private string ClassLabel(string name) =>
        (_context.Session.Setting("classes") as SizeTableModel)?.Control.Rows
            .FirstOrDefault(spec => spec.Name == name)?.Label ?? name;
}
