using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// The Classes or Presets table of the Sizes page (DESIGN.md D5): read-only rows to compare, each
/// with Edit…, and Reset or Delete; the Fits now column follows allotd's fit preview. A preset
/// table ends in Add preset…. The table scrolls sideways in a narrow window.
/// </summary>
internal sealed class SizeTableCard
{
    private const string ResetGlyph = "";
    private const string DeleteGlyph = "";

    private static readonly string[] Headers =
    [
        "Cores", "Minimum cores", "Memory", "GPU", "Video memory", "Run time", "Fits now",
    ];

    private readonly PageContext _context;
    private readonly FitPreviews _fits;
    private readonly SizeTableModel _model;
    private readonly Grid _grid = new() { ColumnSpacing = 20, RowSpacing = 8 };
    private readonly Dictionary<string, RowCells> _rows = new(StringComparer.Ordinal);
    private readonly Button? _add;
    private List<string> _shownNames = [];

    public SizeTableCard(PageContext context, FitPreviews fits, SizeTableModel model)
    {
        _context = context;
        _fits = fits;
        _model = model;
        AutomationProperties.SetName(_grid, model.Label);
        var panel = new StackPanel
        {
            Spacing = 12,
            Children =
            {
                SettingsUi.Text(model.Spec.Help, "SettingsHelpTextStyle"),
                new ScrollViewer
                {
                    Content = _grid,
                    HorizontalScrollMode = ScrollMode.Auto,
                    HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
                    VerticalScrollMode = ScrollMode.Disabled,
                    VerticalScrollBarVisibility = ScrollBarVisibility.Disabled,
                },
            },
        };
        if (model.CanAdd)
        {
            _add = SettingsUi.Button("Add preset…", () => _ = EditAsync(null));
            panel.Children.Add(_add);
        }

        Root = new Border { Style = SettingsUi.Style("SettingsCardStyle"), Child = panel };
        model.PropertyChanged += (_, _) => Sync();
        context.Session.PropertyChanged += (_, _) => Sync();
        fits.Updated += Sync;
        Sync();
    }

    public UIElement Root { get; }

    // Rows are rebuilt only when the set of names changes, so focus stays on a row's button
    // while its values or fits update.
    private void Sync()
    {
        var views = _model.Rows;
        var names = views.Select(view => view.Row.Name).ToList();
        if (!names.SequenceEqual(_shownNames))
        {
            _shownNames = names;
            Rebuild(views);
        }

        var editable = !_context.Session.ReadOnly;
        foreach (var view in views)
        {
            _rows[view.Row.Name].Show(view, editable);
        }

        if (_add is not null)
        {
            _add.IsEnabled = editable;
        }
    }

    private void Rebuild(IReadOnlyList<SizeRowView> views)
    {
        _grid.Children.Clear();
        _grid.RowDefinitions.Clear();
        _grid.ColumnDefinitions.Clear();
        _rows.Clear();
        for (var column = 0; column < Headers.Length + 2; column++)
        {
            _grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        }

        _grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        Place(Header(_model.CanAdd ? "Preset" : "Class"), 0, 0);
        for (var i = 0; i < Headers.Length; i++)
        {
            Place(Header(Headers[i]), 0, i + 1);
        }

        var row = 1;
        foreach (var view in views)
        {
            _grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            _rows[view.Row.Name] = new RowCells(this, view, row++);
        }
    }

    // Screen readers read each row's spoken name instead of the headers and cells.
    private static TextBlock Header(string text)
    {
        var header = SettingsUi.Text(text, "SettingsHelpTextStyle");
        header.TextWrapping = TextWrapping.NoWrap;
        AutomationProperties.SetAccessibilityView(header, AccessibilityView.Raw);
        return header;
    }

    private void Place(FrameworkElement element, int row, int column)
    {
        element.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetRow(element, row);
        Grid.SetColumn(element, column);
        _grid.Children.Add(element);
    }

    private async Task EditAsync(string? name)
    {
        var draft = name is null ? _model.Add() : _model.Edit(name);
        if (draft is null)
        {
            return;
        }

        var view = _model.Rows.FirstOrDefault(candidate => candidate.Row.Name == name);
        var offerReset = view is { BuiltIn: true, Changed: true };
        var outcome = await SizeDialog.ShowAsync(_context, _model, draft, offerReset)
            .ConfigureAwait(true);
        if (outcome == SizeDialogOutcome.Reset && view is not null)
        {
            Remove(view);
        }
    }

    private async Task DeleteAsync(SizeRowView view)
    {
        if (await _context.Confirm(SizesText.DeletePreset(view.Row.Name)).ConfigureAwait(true))
        {
            Remove(view);
        }
    }

    // A reset of a built-in row or the delete of the user's own, with Undo.
    private void Remove(SizeRowView view)
    {
        var old = view.Row;
        _model.Remove(old.Name);
        _context.Toast(SizesText.RowRemoved(_model, view), () => _model.Restore(old));
    }

    /// <summary>
    /// The cells and buttons of one row, kept while the row's name stays.
    /// </summary>
    private sealed class RowCells
    {
        private readonly SizeTableCard _card;
        private readonly TextBlock _name;
        private readonly TextBlock[] _values = new TextBlock[Headers.Length];
        private readonly Button _edit;
        private readonly Button? _reset;
        private readonly Button? _delete;

        public RowCells(SizeTableCard card, SizeRowView view, int row)
        {
            _card = card;
            var name = view.Row.Name;
            var (cell, spoken) = NameCell(view, card._model.CanAdd);
            _name = spoken;
            card.Place(cell, row, 0);
            for (var i = 0; i < _values.Length; i++)
            {
                _values[i] = new TextBlock { TextWrapping = TextWrapping.NoWrap };
                AutomationProperties.SetAccessibilityView(_values[i], AccessibilityView.Raw);
                card.Place(_values[i], row, i + 1);
            }

            _edit = SettingsUi.Button("Edit…", () => _ = card.EditAsync(name));
            AutomationProperties.SetName(_edit, SizesText.EditName(card._model, view));
            var actions = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 4,
                Children = { _edit },
            };
            if (view.BuiltIn)
            {
                _reset = SettingsUi.IconButton(
                    ResetGlyph,
                    SizesText.ResetName(card._model, view),
                    () => card.Remove(Current() ?? view));
                actions.Children.Add(_reset);
            }
            else
            {
                _delete = SettingsUi.IconButton(
                    DeleteGlyph,
                    SizesText.DeleteName(view),
                    () => _ = card.DeleteAsync(Current() ?? view));
                actions.Children.Add(_delete);
            }

            card.Place(actions, row, _values.Length + 1);

            SizeRowView? Current() =>
                card._model.Rows.FirstOrDefault(candidate => candidate.Row.Name == name);
        }

        public void Show(SizeRowView view, bool editable)
        {
            var number = _card._context.Number;
            var cells = SizesText.Cells(view.Row, number);
            var fit = _card._fits.Of(_card._model.Key, view.Row.Name);
            var fits = SizesText.FitsNow(fit, number);
            var label = _card._model.CanAdd ? view.Row.Name : view.Label;
            AutomationProperties.SetName(_name, SizesText.Spoken(label, view.Row, fit, number));
            string[] texts =
            [
                cells.Cores, cells.MinCores, cells.Memory, cells.Gpu, cells.VideoMemory,
                cells.RunTime, fits,
            ];
            for (var i = 0; i < texts.Length; i++)
            {
                _values[i].Text = texts[i];
            }

            _edit.IsEnabled = editable;
            if (_reset is not null)
            {
                _reset.Visibility = SettingsUi.Shows(view.Changed);
                _reset.IsEnabled = editable;
            }

            if (_delete is not null)
            {
                _delete.IsEnabled = editable;
            }
        }

        // A class shows its label over its key; a preset its name over "built in" or "yours". The
        // top line is the one screen readers read, named with the whole row.
        private static (StackPanel Cell, TextBlock Spoken) NameCell(SizeRowView view, bool preset)
        {
            var main = new TextBlock { Text = preset ? view.Row.Name : view.Label };
            var sub = SettingsUi.Text(
                preset ? (view.BuiltIn ? "built in" : "yours") : view.Row.Name,
                "SettingsHelpTextStyle");
            if (preset)
            {
                main.FontFamily = SettingsUi.Mono;
            }
            else
            {
                sub.FontFamily = SettingsUi.Mono;
            }

            AutomationProperties.SetAccessibilityView(sub, AccessibilityView.Raw);
            return (new StackPanel { Children = { main, sub } }, main);
        }
    }
}
