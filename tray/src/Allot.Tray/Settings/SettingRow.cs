using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Media;

namespace Allot.Tray.Settings;

/// <summary>
/// What the line under a row says, if anything.
/// </summary>
internal enum RowNoteKind
{
    /// <summary>
    /// The value cannot be saved: error style with an icon.
    /// </summary>
    Error,

    /// <summary>
    /// Saved, but it waits for something, such as a restart.
    /// </summary>
    Caution,

    /// <summary>
    /// Why the row is disabled.
    /// </summary>
    Info,
}

/// <summary>
/// The line under a row.
/// </summary>
/// <param name="Kind">How it looks.</param>
/// <param name="Text">What it says.</param>
/// <param name="Undo">An Undo link after it, or <c>null</c>.</param>
internal sealed record RowNote(RowNoteKind Kind, string Text, Action? Undo = null);

/// <summary>
/// One setting (DESIGN.md section 3): label and control, the help line ending in the TOML key, an
/// optional reset button, and an optional line under it for an error or a note. Screen readers get
/// the label as the control's name and the help with the key as its description.
/// </summary>
internal sealed partial class SettingRow : UserControl
{
    private const string ErrorGlyph = "";
    private const string ResetGlyph = "";

    private readonly FrameworkElement _control;
    private readonly string _description;
    private readonly Button _reset;
    private readonly StackPanel _noteLine;
    private readonly FontIcon _noteIcon = SettingsUi.Decorative(new FontIcon
    {
        Style = SettingsUi.Style("SettingsErrorIconStyle"),
    });
    private readonly TextBlock _note = new();
    private readonly HyperlinkButton _undo = new()
    {
        Content = "Undo",
        Padding = new Thickness(4, 0, 4, 0),
    };
    private Action? _onReset;
    private Action? _onUndo;

    /// <param name="label">The setting's label.</param>
    /// <param name="help">One or two plain sentences.</param>
    /// <param name="key">The TOML key, or <c>null</c> for a panel setting.</param>
    /// <param name="control">The control, or the panel that holds it with its companions.</param>
    /// <param name="field">The element screen readers reach for the setting, when
    /// <paramref name="control"/> is a panel (which they do not see): named after the label
    /// unless it has a name of its own, and described by the help and key.</param>
    public SettingRow(
        string label,
        string help,
        string? key,
        FrameworkElement control,
        FrameworkElement? field = null)
    {
        _control = field ?? control;
        if (string.IsNullOrEmpty(AutomationProperties.GetName(_control)))
        {
            AutomationProperties.SetName(_control, label);
        }

        var description = key is null ? help : $"{help} {key}";
        _description = description;
        if (description.Length > 0)
        {
            AutomationProperties.SetHelpText(_control, description);
        }

        var grid = new Grid { ColumnSpacing = 12, RowSpacing = 2 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = SettingsUi.Star });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(32) });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var title = new TextBlock
        {
            Text = label,
            Style = SettingsUi.Style("BodyTextBlockStyle"),
            TextWrapping = TextWrapping.WrapWholeWords,
            VerticalAlignment = VerticalAlignment.Center,
        };
        AutomationProperties.SetAccessibilityView(title, AccessibilityView.Raw);
        grid.Children.Add(title);

        if (description.Length > 0)
        {
            var helpText = new TextBlock { Style = SettingsUi.Style("SettingsHelpTextStyle") };
            helpText.Inlines.Add(new Run { Text = help });
            if (key is not null)
            {
                helpText.Inlines.Add(new Run { Text = "  " });
                helpText.Inlines.Add(new Run
                {
                    Text = key,
                    FontFamily = new FontFamily("Cascadia Mono, Consolas"),
                    FontSize = 11,
                });
            }

            AutomationProperties.SetAccessibilityView(helpText, AccessibilityView.Raw);
            Grid.SetRow(helpText, 1);
            grid.Children.Add(helpText);
        }

        control.VerticalAlignment = VerticalAlignment.Center;
        Grid.SetColumn(control, 1);
        Grid.SetRowSpan(control, 2);
        grid.Children.Add(control);

        _reset = new Button
        {
            Width = 32,
            Height = 32,
            Padding = new Thickness(0),
            Visibility = Visibility.Collapsed,
            VerticalAlignment = VerticalAlignment.Center,
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            BorderThickness = new Thickness(0),
            Content = new FontIcon
            {
                Glyph = ResetGlyph,
                FontSize = 14,
                IsTextScaleFactorEnabled = false,
                FontFamily = SettingsUi.SymbolFont,
            },
        };
        ToolTipService.SetToolTip(_reset, "Reset to default");
        _reset.Click += (_, _) => _onReset?.Invoke();
        Grid.SetColumn(_reset, 2);
        Grid.SetRowSpan(_reset, 2);
        grid.Children.Add(_reset);

        _undo.Click += (_, _) => _onUndo?.Invoke();
        _noteLine = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 6,
            Visibility = Visibility.Collapsed,
            Children = { _noteIcon, _note, _undo },
        };
        _note.TextWrapping = TextWrapping.WrapWholeWords;
        Grid.SetRow(_noteLine, 2);
        Grid.SetColumnSpan(_noteLine, 2);
        grid.Children.Add(_noteLine);

        Content = grid;
    }

    /// <summary>
    /// Indents the row under the one before it, such as Idle for under its switch.
    /// </summary>
    public bool Indented
    {
        set => Padding = new Thickness(value ? 24 : 0, 0, 0, 0);
    }

    /// <summary>
    /// Shows or hides the reset button.
    /// </summary>
    /// <param name="name">The button's accessible name, such as "Reset Headroom to 15 %".</param>
    /// <param name="onReset">What the button does; <c>null</c> hides it.</param>
    public void SetReset(string name, Action? onReset)
    {
        _onReset = onReset;
        _reset.Visibility = SettingsUi.Shows(onReset is not null);
        AutomationProperties.SetName(_reset, name);
    }

    /// <summary>
    /// Shows the line under the row, or hides it for <c>null</c>. An error also leads the
    /// control's description while it shows; the window announces it (SettingsAnnouncer).
    /// </summary>
    public void SetNote(RowNote? note)
    {
        var error = note is { Kind: RowNoteKind.Error } ? note.Text : null;
        AutomationProperties.SetHelpText(
            _control,
            error is null ? _description : $"{error} {_description}".TrimEnd());
        if (note is null)
        {
            _noteLine.Visibility = Visibility.Collapsed;
            _note.Text = string.Empty;
            _onUndo = null;
            return;
        }

        _note.Text = note.Text;
        _note.Style = SettingsUi.Style(note.Kind switch
        {
            RowNoteKind.Error => "SettingsErrorTextStyle",
            RowNoteKind.Caution => "SettingsCautionTextStyle",
            _ => "SettingsHelpTextStyle",
        });
        _noteIcon.Visibility = SettingsUi.Shows(note.Kind == RowNoteKind.Error);
        _onUndo = note.Undo;
        _undo.Visibility = SettingsUi.Shows(note.Undo is not null);
        _noteLine.Visibility = Visibility.Visible;
    }
}
