using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// The Edit dialog of a class or preset, and Add preset (DESIGN.md section 4, Sizes): the name
/// for a preset the user adds or renames, then every size field, the live line from allotd's fit
/// preview for the values in the dialog, and Save, which writes the whole row at once.
/// </summary>
internal sealed class SizeDialog
{
    private readonly PageContext _context;
    private readonly SizeTableModel _table;
    private readonly RowDraft _draft;
    private readonly List<Action> _syncs = [];
    private readonly TextBlock _warning = SettingsUi.Text(string.Empty, "SettingsCautionTextStyle");
    private readonly TextBlock _live = SettingsUi.LiveSentence();
    private readonly DispatcherQueueTimer _debounce;
    private int _request;

    private SizeDialog(PageContext context, SizeTableModel table, RowDraft draft)
    {
        _context = context;
        _table = table;
        _draft = draft;
        _debounce = DispatcherQueue.GetForCurrentThread().CreateTimer();
        _debounce.Interval = SettingsSession.SaveDelay;
        _debounce.IsRepeating = false;
        _debounce.Tick += (_, _) => _ = PreviewAsync();
    }

    /// <summary>
    /// Shows the dialog for <paramref name="draft"/>.
    /// </summary>
    /// <param name="context">The page's context.</param>
    /// <param name="table">The table the row belongs to.</param>
    /// <param name="draft">The row being edited, or a new preset.</param>
    /// <param name="offerReset">Whether the footer offers Reset to built-in size.</param>
    public static async Task<SizeDialogOutcome> ShowAsync(
        PageContext context,
        SizeTableModel table,
        RowDraft draft,
        bool offerReset)
    {
        var dialog = new SizeDialog(context, table, draft);
        try
        {
            return await dialog.RunAsync(offerReset).ConfigureAwait(true);
        }
        finally
        {
            dialog._debounce.Stop();
        }
    }

    private async Task<SizeDialogOutcome> RunAsync(bool offerReset)
    {
        var fields = new StackPanel { Spacing = 4 };
        if (_draft.CanRename && _draft.NameSpec is { } nameSpec)
        {
            fields.Children.Add(NameRow(nameSpec.Label, nameSpec.Help));
        }

        foreach (var field in _draft.Fields)
        {
            fields.Children.Add(field.Key == "min_cores" ? MinCoresRow(field) : NumberRow(field));
        }

        _warning.Visibility = Visibility.Collapsed;
        fields.Children.Add(_warning);
        fields.Children.Add(_live);
        var dialog = new ContentDialog
        {
            Title = SizesText.DialogTitle(_table, _draft),
            Content = fields,
            PrimaryButtonText = "Save",
            CloseButtonText = "Cancel",
            SecondaryButtonText = offerReset ? "Reset to built-in size" : string.Empty,
            DefaultButton = ContentDialogButton.None,
        };
        dialog.Resources["ContentDialogMaxWidth"] = 720.0;
        dialog.PrimaryButtonClick += (_, args) => args.Cancel = !_table.Save(_draft);

        _draft.PropertyChanged += (_, _) => OnDraftChanged();
        foreach (var field in _draft.Fields)
        {
            field.PropertyChanged += (_, _) => OnDraftChanged();
        }

        Sync();
        _ = PreviewAsync();
        return await _context.Dialog(dialog).ConfigureAwait(true) switch
        {
            ContentDialogResult.Primary => SizeDialogOutcome.Saved,
            ContentDialogResult.Secondary => SizeDialogOutcome.Reset,
            _ => SizeDialogOutcome.Closed,
        };
    }

    private SettingRow NameRow(string label, string help)
    {
        var box = new TextBox
        {
            Text = _draft.Name,
            Width = 240,
            FontFamily = SettingsUi.Mono,
            IsSpellCheckEnabled = false,
        };
        box.TextChanged += (_, _) => _draft.SetName(box.Text);
        var row = new SettingRow(label, help, null, box);
        _syncs.Add(() => row.SetNote(IssueNote(_draft.NameIssue)));
        return row;
    }

    private SettingRow NumberRow(NumberModel field)
    {
        var editor = new NumberEditor(field);
        var row = new SettingRow(field.Label, field.Spec.Help, null, editor.Root);
        _syncs.Add(() =>
        {
            editor.Sync(true);
            row.SetNote(IssueNote(field.Issue));
        });
        return row;
    }

    // Automatic ("half the cores, at least 1") or Custom with its own number.
    private SettingRow MinCoresRow(NumberModel field)
    {
        var group = $"min-cores-{Guid.NewGuid():N}";
        var automatic = new RadioButton { GroupName = group };
        var custom = new RadioButton { GroupName = group, Content = "Custom" };
        var editor = new NumberEditor(field);
        AutomationProperties.SetName(editor.Root, field.Label);
        var customLine = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { custom, editor.Root },
        };
        var choice = new StackPanel { Spacing = 4, Children = { automatic, customLine } };
        var syncing = false;
        automatic.Checked += (_, _) =>
        {
            if (!syncing && !field.Automatic)
            {
                field.SetAutomatic(true);
            }
        };
        custom.Checked += (_, _) =>
        {
            if (!syncing && field.Automatic)
            {
                field.SetAutomatic(false, AutomaticCores());
            }
        };
        var row = new SettingRow(field.Label, field.Spec.Help, null, choice);
        _syncs.Add(() =>
        {
            syncing = true;
            var rule = field.Control.Automatic ?? string.Empty;
            automatic.Content = $"Automatic ({_context.Number(AutomaticCores())}: {rule})";
            automatic.IsChecked = field.Automatic;
            custom.IsChecked = !field.Automatic;
            syncing = false;
            editor.Sync(!field.Automatic);
            row.SetNote(IssueNote(field.Issue));
        });
        return row;
    }

    private double AutomaticCores() =>
        SizesText.AutomaticMinCores(_draft.Field("cores")?.Value ?? 0);

    private RowNote? IssueNote(ValueIssue? issue) =>
        issue is null
            ? null
            : new RowNote(RowNoteKind.Error, ValueText.IssueText(issue, _context.Number));

    private void OnDraftChanged()
    {
        Sync();
        _debounce.Stop();
        _debounce.Start();
    }

    private void Sync()
    {
        foreach (var sync in _syncs)
        {
            sync();
        }

        var warning = SizesText.RenameWarning(_draft) ?? string.Empty;
        _warning.Text = warning;
        _warning.Visibility = SettingsUi.Shows(warning.Length > 0);
    }

    private async Task PreviewAsync()
    {
        var session = _context.Session;
        if (session.Status != SessionStatus.Ready)
        {
            SettingsUi.SetLive(_live, LiveText.ServiceStopped);
            return;
        }

        if (_table.Proposal(_draft) is not { } changes)
        {
            SettingsUi.SetLive(_live, string.Empty);
            return;
        }

        var request = ++_request;
        var name = _draft.Name;
        var reply = await session.PreviewAsync(changes).ConfigureAwait(true);
        if (request != _request)
        {
            return;
        }

        var fits = _table.Key == "classes" ? reply.Value?.Classes : reply.Value?.Presets;
        var fit = fits?.FirstOrDefault(candidate => candidate.Name == name);
        SettingsUi.SetLive(
            _live,
            fit is null ? string.Empty : SizesText.DialogFits(fit, _context.Number));
    }
}

/// <summary>
/// What a <see cref="SizeDialog"/> ended with.
/// </summary>
internal enum SizeDialogOutcome
{
    /// <summary>
    /// Cancelled, or another dialog was open.
    /// </summary>
    Closed,

    /// <summary>
    /// The row was handed over to be written.
    /// </summary>
    Saved,

    /// <summary>
    /// The user asked to reset the row to its built-in size.
    /// </summary>
    Reset,
}
