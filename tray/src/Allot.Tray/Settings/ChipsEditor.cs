using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

using Windows.Foundation;
using Windows.System;

namespace Allot.Tray.Settings;

/// <summary>
/// A string list as chips with remove buttons, then a box and an Add button (DESIGN.md section 3,
/// String list): the print-only flags, and a rule's arguments in its dialog. Each add or remove
/// goes to the model at once; the caller shows the model's issue.
/// </summary>
internal sealed class ChipsEditor
{
    private const string RemoveGlyph = "";

    private readonly StringListModel _model;
    private readonly WrapPanel _chips = new();
    private readonly TextBox _entry;
    private readonly Button _add;
    private IReadOnlyList<string>? _shown;
    private bool _enabled = true;

    /// <param name="model">The list.</param>
    /// <param name="entryName">The box's accessible name, such as "New print-only flag".</param>
    /// <param name="placeholder">An example entry.</param>
    /// <param name="addLabel">The Add button, such as "Add flag".</param>
    public ChipsEditor(StringListModel model, string entryName, string placeholder, string addLabel)
    {
        _model = model;
        AutomationProperties.SetName(_chips, model.Label);
        _entry = new TextBox
        {
            PlaceholderText = placeholder,
            MinWidth = 160,
            FontFamily = SettingsUi.Mono,
            IsSpellCheckEnabled = false,
        };
        AutomationProperties.SetName(_entry, entryName);
        _entry.KeyDown += OnEntryKeyDown;
        _add = SettingsUi.Button(addLabel, AddEntry);
        var entryLine = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { _entry, _add },
        };
        Root = new StackPanel { Spacing = 8, Children = { _chips, entryLine } };
    }

    public FrameworkElement Root { get; }

    /// <summary>
    /// The entry box, which keeps its own name and takes the row's help as its description.
    /// </summary>
    public FrameworkElement Field => _entry;

    /// <summary>
    /// Shows the model's entries; rebuilds the chips only when they changed, so focus stays.
    /// </summary>
    public void Sync(bool enabled)
    {
        var entries = _model.Value;
        if (_shown is null || !_shown.SequenceEqual(entries) || enabled != _enabled)
        {
            _shown = entries;
            _enabled = enabled;
            _chips.Children.Clear();
            for (var i = 0; i < entries.Count; i++)
            {
                _chips.Children.Add(Chip(entries[i], i, enabled));
            }
        }

        _chips.Visibility = SettingsUi.Shows(entries.Count > 0);
        _entry.IsEnabled = enabled;
        _add.IsEnabled = enabled;
    }

    private Border Chip(string entry, int index, bool enabled)
    {
        var remove = SettingsUi.IconButton(
            RemoveGlyph,
            $"Remove {entry}",
            () => _model.Remove(index),
            size: 24);
        remove.IsEnabled = enabled;
        var text = new TextBlock
        {
            Text = entry,
            FontFamily = SettingsUi.Mono,
            VerticalAlignment = VerticalAlignment.Center,
        };
        return new Border
        {
            Style = SettingsUi.Style("SettingsChipStyle"),
            Child = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                Spacing = 2,
                Children = { text, remove },
            },
        };
    }

    private void AddEntry()
    {
        if (_model.Add(_entry.Text))
        {
            _entry.Text = string.Empty;
        }

        _ = _entry.Focus(FocusState.Programmatic);
    }

    private void OnEntryKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Enter)
        {
            AddEntry();
            args.Handled = true;
        }
    }
}

/// <summary>
/// Lays its children in lines, starting a new line when the next one does not fit.
/// </summary>
internal sealed partial class WrapPanel : Panel
{
    private const double Gap = 6;

    protected override Size MeasureOverride(Size availableSize)
    {
        var width = 0.0;
        var x = 0.0;
        var y = 0.0;
        var line = 0.0;
        foreach (var child in Children)
        {
            child.Measure(new Size(availableSize.Width, double.PositiveInfinity));
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > availableSize.Width)
            {
                y += line + Gap;
                x = 0;
                line = 0;
            }

            x += size.Width + Gap;
            line = Math.Max(line, size.Height);
            width = Math.Max(width, x - Gap);
        }

        return new Size(width, y + line);
    }

    protected override Size ArrangeOverride(Size finalSize)
    {
        var x = 0.0;
        var y = 0.0;
        var line = 0.0;
        foreach (var child in Children)
        {
            var size = child.DesiredSize;
            if (x > 0 && x + size.Width > finalSize.Width)
            {
                y += line + Gap;
                x = 0;
                line = 0;
            }

            child.Arrange(new Rect(x, y, size.Width, size.Height));
            x += size.Width + Gap;
            line = Math.Max(line, size.Height);
        }

        return finalSize;
    }
}
