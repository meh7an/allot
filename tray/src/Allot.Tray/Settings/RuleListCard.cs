using System.Globalization;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Documents;
using Microsoft.UI.Xaml.Input;

using Windows.System;

namespace Allot.Tray.Settings;

/// <summary>
/// The Commands page: Test a command above the heavy-command rules (DESIGN.md D6), and the rules
/// as a numbered list with a menu per rule, Alt+Up and Alt+Down to move the focused rule and
/// Delete to delete it, then Add rule… and Use built-in rules…. Every change writes the whole
/// list at once.
/// </summary>
internal sealed class RuleListCard
{
    private const string MoreGlyph = "";

    private readonly PageContext _context;
    private readonly RuleListModel _model;
    private readonly ShellChoice _shell = new();
    private readonly CommandTestBox _test;
    private readonly TextBlock _status =
        new() { Style = SettingsUi.Style("SettingsHelpTextStyle") };
    private readonly ListView _list = new()
    {
        SelectionMode = ListViewSelectionMode.None,
        IsItemClickEnabled = false,
    };
    private readonly TextBlock _empty = SettingsUi.Text(RulesText.Empty, "BodyTextBlockStyle");
    private readonly Button _add;
    private readonly Button _builtIn;
    private readonly List<RuleRow> _rows = [];
    private IReadOnlySet<int> _matched = new HashSet<int>();
    private string _shown = string.Empty;
    private int? _focusAfterBuild;

    public RuleListCard(PageContext context, RuleListModel model)
    {
        _context = context;
        _model = model;
        _test = new CommandTestBox(context, CommandTestBox.Help, PageScope, _shell);
        _test.Matched += matched =>
        {
            _matched = matched;
            ShowMatches();
        };
        AutomationProperties.SetName(_list, "Rules, checked in order");
        _add = SettingsUi.Button("Add rule…", () => _ = EditAsync(null));
        _builtIn = SettingsUi.Button("Use built-in rules…", () => _ = UseBuiltInAsync());
        var footer = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Spacing = 8,
            Children = { _add, _builtIn },
        };
        Rules = new Border
        {
            Style = SettingsUi.Style("SettingsCardStyle"),
            Child = new StackPanel { Spacing = 8, Children = { _status, _list, _empty, footer } },
        };
        TestCard = new StackPanel
        {
            Children =
            {
                SettingsUi.Heading("Test a command"),
                new Border { Style = SettingsUi.Style("SettingsCardStyle"), Child = _test.Root },
            },
        };
        model.PropertyChanged += (_, _) =>
        {
            Sync();
            _test.Retest();
        };
        context.Session.PropertyChanged += (_, _) => Sync();
        Sync();
    }

    /// <summary>
    /// The Test a command group, heading included, which goes above the rules.
    /// </summary>
    public UIElement TestCard { get; }

    /// <summary>
    /// The card of the Heavy-command rules group.
    /// </summary>
    public UIElement Rules { get; }

    private bool Editable => !_context.Session.ReadOnly;

    private Func<JobClass, string> ClassLabel =>
        RulesText.ClassLabels((_context.Session.Setting("classes") as SizeTableModel)?.Control);

    private TestScope PageScope() => new(null, _model.Value);

    // The list is rebuilt only when the rules or the read-only state changed, so focus stays
    // on a rule through refreshes.
    private void Sync()
    {
        var rules = _model.Value;
        var editable = Editable;
        var shown = Signature(rules, editable);
        if (shown != _shown)
        {
            _shown = shown;
            Rebuild(rules, editable);
        }

        _status.Inlines.Clear();
        _status.Inlines.Add(new Run { Text = RulesText.Status(_model) });
        _status.Inlines.Add(new Run { Text = "  " });
        _status.Inlines.Add(new Run
        {
            Text = _model.Key,
            FontFamily = SettingsUi.Mono,
            FontSize = 11,
        });
        _list.Visibility = SettingsUi.Shows(rules.Count > 0);
        _empty.Visibility = SettingsUi.Shows(rules.Count == 0);
        _add.IsEnabled = editable;
        _builtIn.IsEnabled = editable && _model.CanReset;
    }

    private static string Signature(IReadOnlyList<HeavyRule> rules, bool editable) =>
        string.Join(
            "\n",
            rules.Select(rule => $"{rule.Program}\t{string.Join('\t', rule.Args)}\t{rule.Class}"))
            + (editable ? "\nE" : "\nR");

    private void Rebuild(IReadOnlyList<HeavyRule> rules, bool editable)
    {
        _list.Items.Clear();
        _rows.Clear();
        for (var i = 0; i < rules.Count; i++)
        {
            var row = new RuleRow(this, rules[i], i, rules.Count, editable);
            _rows.Add(row);
            _list.Items.Add(row.Item);
        }

        ShowMatches();
        if (_focusAfterBuild is { } focus && _rows.Count > 0)
        {
            _focusAfterBuild = null;
            var item = _rows[Math.Clamp(focus, 0, _rows.Count - 1)].Item;
            _ = _list.DispatcherQueue.TryEnqueue(() => item.Focus(FocusState.Keyboard));
        }
    }

    private void ShowMatches()
    {
        foreach (var row in _rows)
        {
            row.ShowMatch(_matched.Contains(row.Index));
        }
    }

    private async Task EditAsync(int? index)
    {
        var draft = index is { } at ? _model.Edit(at) : _model.Add();
        if (draft is not null)
        {
            _ = await RuleDialog.ShowAsync(_context, _model, draft, _shell).ConfigureAwait(true);
            _test.ShowShell();
        }
    }

    private void Move(int index, int to)
    {
        if (!Editable || to < 0 || to >= _model.Value.Count)
        {
            return;
        }

        _focusAfterBuild = to;
        _model.Move(index, to);
    }

    private void Delete(int index)
    {
        if (!Editable)
        {
            return;
        }

        var old = _model.Value;
        _focusAfterBuild = index;
        _model.Remove(index);
        _context.Toast(RulesText.Deleted(index), () => _model.Replace(old));
    }

    private async Task UseBuiltInAsync()
    {
        var confirmation = RulesText.UseBuiltIn(_model.Value.Count, _model.Control.Default.Count);
        if (await _context.Confirm(confirmation).ConfigureAwait(true))
        {
            _model.Reset();
            await _context.Session.FlushAsync().ConfigureAwait(true);
        }
    }

    /// <summary>
    /// One rule: number, program and arguments, the Matches tag, its class, and its menu.
    /// </summary>
    private sealed class RuleRow
    {
        private readonly Border _back;
        private readonly Border _tag;
        private readonly string _name;

        public RuleRow(RuleListCard card, HeavyRule rule, int index, int count, bool editable)
        {
            Index = index;
            _name = RulesText.RowName(index, rule, card.ClassLabel);
            var grid = new Grid { ColumnSpacing = 12, Padding = new Thickness(8, 6, 4, 6) };
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(24) });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = SettingsUi.Star });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

            var number = SettingsUi.Text(
                (index + 1).ToString(CultureInfo.CurrentCulture),
                "SettingsHelpTextStyle");
            var program = new TextBlock { Text = rule.Program, FontFamily = SettingsUi.Mono };
            var args = SettingsUi.Text(RulesText.Arguments(rule), "SettingsHelpTextStyle");
            args.FontFamily = SettingsUi.Mono;
            var main = new StackPanel { Children = { program, args } };
            _tag = new Border
            {
                Style = SettingsUi.Style("SettingsTagStyle"),
                Child = SettingsUi.Text("Matches", "SettingsTagTextStyle"),
                Visibility = Visibility.Collapsed,
            };
            var target = SettingsUi.Text($"→ {card.ClassLabel(rule.Class)}", "BodyTextBlockStyle");
            var more = SettingsUi.IconButton(
                MoreGlyph,
                RulesText.MoreOptionsName(index),
                () => { });
            more.Flyout = Menu(card, index, count);
            more.IsEnabled = editable;

            UIElement[] cells = [number, main, _tag, target, more];
            for (var column = 0; column < cells.Length; column++)
            {
                var cell = (FrameworkElement)cells[column];
                cell.VerticalAlignment = VerticalAlignment.Center;
                Grid.SetColumn(cell, column);
                grid.Children.Add(cell);
            }

            _back = new Border { Style = SettingsUi.Style("SettingsPlainRowStyle"), Child = grid };
            Item = new ListViewItem
            {
                Content = _back,
                HorizontalContentAlignment = HorizontalAlignment.Stretch,
                Padding = new Thickness(0),
                KeyboardAcceleratorPlacementMode = KeyboardAcceleratorPlacementMode.Hidden,
            };
            AutomationProperties.SetName(Item, _name);
            if (editable)
            {
                // The row menu from the keyboard too (Shift+F10 or the Menu key), since a list
                // item's own button may be out of the Tab order.
                Item.ContextFlyout = Menu(card, index, count);
                Accelerate(Item, VirtualKey.Up, VirtualKeyModifiers.Menu, () =>
                    card.Move(index, index - 1));
                Accelerate(Item, VirtualKey.Down, VirtualKeyModifiers.Menu, () =>
                    card.Move(index, index + 1));
                Accelerate(Item, VirtualKey.Delete, VirtualKeyModifiers.None, () =>
                    card.Delete(index));
            }
        }

        public int Index { get; }

        public ListViewItem Item { get; }

        public void ShowMatch(bool matched)
        {
            _tag.Visibility = SettingsUi.Shows(matched);
            _back.Style = SettingsUi.Style(
                matched ? "SettingsMatchedRowStyle" : "SettingsPlainRowStyle");
            AutomationProperties.SetName(Item, matched ? $"{_name}, matches" : _name);
        }

        private static MenuFlyout Menu(RuleListCard card, int index, int count)
        {
            var edit = new MenuFlyoutItem { Text = "Edit…" };
            edit.Click += (_, _) => _ = card.EditAsync(index);
            var up = new MenuFlyoutItem { Text = "Move up", IsEnabled = index > 0 };
            up.Click += (_, _) => card.Move(index, index - 1);
            var down = new MenuFlyoutItem { Text = "Move down", IsEnabled = index < count - 1 };
            down.Click += (_, _) => card.Move(index, index + 1);
            var delete = new MenuFlyoutItem { Text = "Delete" };
            delete.Click += (_, _) => card.Delete(index);
            var menu = new MenuFlyout();
            menu.Items.Add(edit);
            menu.Items.Add(up);
            menu.Items.Add(down);
            menu.Items.Add(new MenuFlyoutSeparator());
            menu.Items.Add(delete);
            return menu;
        }

        // Only while focus is on this rule, so Delete in a text box never deletes a rule.
        private static void Accelerate(
            ListViewItem item,
            VirtualKey key,
            VirtualKeyModifiers modifiers,
            Action action)
        {
            var accelerator = new KeyboardAccelerator
            {
                Key = key,
                Modifiers = modifiers,
                ScopeOwner = item,
            };
            accelerator.Invoked += (_, args) =>
            {
                args.Handled = true;
                action();
            };
            item.KeyboardAccelerators.Add(accelerator);
        }
    }
}
