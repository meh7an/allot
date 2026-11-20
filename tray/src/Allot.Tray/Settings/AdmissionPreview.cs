using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// The Admission page's live parts from allotd's fit preview: the headroom sentence and the What
/// fits on this machine table.
/// </summary>
internal sealed class AdmissionPreview
{
    public const string FitsGroup = "What fits on this machine";

    private readonly PageContext _context;
    private readonly FitPreviews _fits;
    private readonly Grid _table = new() { ColumnSpacing = 24, RowSpacing = 8 };
    private readonly TextBlock _caption = SettingsUi.LiveSentence();

    public AdmissionPreview(PageContext context, FitPreviews fits)
    {
        _context = context;
        _fits = fits;
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
        fits.Updated += Show;
        Show();
    }

    public UIElement Card { get; }

    /// <summary>
    /// The headroom sentence for the values on screen, or why it is not known.
    /// </summary>
    public string HeadroomText()
    {
        if (_fits.Unavailable is { } reason)
        {
            return reason;
        }

        return _fits.Latest is { } preview
            ? LiveText.Headroom(preview.KeptFree, _context.Number)
            : string.Empty;
    }

    private void Show()
    {
        _table.Children.Clear();
        _table.RowDefinitions.Clear();
        if (_fits.Unavailable is not null || _fits.Latest is not { } preview)
        {
            SettingsUi.SetLive(_caption, _fits.Unavailable ?? FitPreviews.NotYet);
            return;
        }

        AddRow(0, "Class", "Now", "When idle", header: true);
        var row = 1;
        foreach (var fit in preview.Classes)
        {
            AddRow(
                row++,
                ClassLabel(fit.Name),
                LiveText.Fits(fit.Now, fit.MinCores, _context.Number),
                LiveText.Fits(fit.Idle, fit.MinCores, _context.Number),
                header: false);
        }

        SettingsUi.SetLive(_caption, LiveText.FitsCaption(preview.CpuBusyPct, _context.Number));
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
