using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// The rules a command line is tested against: unsaved changes on top of the page's values, and
/// the list they make, for naming the matched rule.
/// </summary>
/// <param name="Changes">Changes beyond the page's, such as the rule list with a dialog's draft;
/// <c>null</c> for none.</param>
/// <param name="Rules">The rules the test runs against.</param>
internal sealed record TestScope(IReadOnlyList<Change>? Changes, IReadOnlyList<HeavyRule> Rules);

/// <summary>
/// The shell the test boxes parse with, shared by the page's box and the rule dialog's.
/// </summary>
internal sealed class ShellChoice
{
    public Shell Value { get; set; } = Shell.Bash;
}

/// <summary>
/// Test a command (DESIGN.md D6): a command line and the shell that runs it, then one line per
/// simple command saying how the hook would treat it. On the Commands page it tests the rules on
/// screen; in the rule dialog, the list with the rule being edited.
/// </summary>
internal sealed class CommandTestBox
{
    public const string Help = "Shows how the hook would treat this command with the rules below.";

    private static readonly TimeSpan Pause = TimeSpan.FromMilliseconds(300);

    private readonly PageContext _context;
    private readonly Func<TestScope?> _scope;
    private readonly ShellChoice _shell;
    private readonly TextBox _line;
    private readonly ComboBox _shellBox = new() { MinWidth = 132 };
    private readonly TextBlock _result = SettingsUi.LiveSentence();
    private readonly DispatcherQueueTimer _debounce;
    private int _request;

    /// <param name="context">The page's context.</param>
    /// <param name="help">The help line under the label.</param>
    /// <param name="scope">The rules to test against now; <c>null</c> while a draft rule is not
    /// complete.</param>
    /// <param name="shell">The shared shell choice.</param>
    public CommandTestBox(
        PageContext context,
        string help,
        Func<TestScope?> scope,
        ShellChoice shell)
    {
        _context = context;
        _scope = scope;
        _shell = shell;
        _line = new TextBox
        {
            PlaceholderText = "cargo build --release",
            FontFamily = SettingsUi.Mono,
            IsSpellCheckEnabled = false,
        };
        AutomationProperties.SetName(_line, "Command line");
        AutomationProperties.SetHelpText(_line, help);
        _line.TextChanged += (_, _) => Retest();
        _shellBox.Items.Add("Bash");
        _shellBox.Items.Add("PowerShell");
        AutomationProperties.SetName(_shellBox, "Shell");
        _shellBox.SelectionChanged += (_, _) =>
        {
            _shell.Value = _shellBox.SelectedIndex == 1 ? Shell.Powershell : Shell.Bash;
            Retest();
        };
        _debounce = DispatcherQueue.GetForCurrentThread().CreateTimer();
        _debounce.Interval = Pause;
        _debounce.IsRepeating = false;
        _debounce.Tick += (_, _) => _ = RunAsync();

        var entry = new Grid { ColumnSpacing = 8 };
        entry.ColumnDefinitions.Add(new ColumnDefinition { Width = SettingsUi.Star });
        entry.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        Grid.SetColumn(_shellBox, 1);
        entry.Children.Add(_line);
        entry.Children.Add(_shellBox);
        _result.Visibility = Visibility.Collapsed;
        _result.IsTextSelectionEnabled = true;
        // The label and help are the box's name and description, so screen readers skip them.
        var label = SettingsUi.Text("Command line", "BodyTextBlockStyle");
        var helpLine = SettingsUi.Text(help, "SettingsHelpTextStyle");
        AutomationProperties.SetAccessibilityView(label, AccessibilityView.Raw);
        AutomationProperties.SetAccessibilityView(helpLine, AccessibilityView.Raw);
        Root = new StackPanel
        {
            Spacing = 4,
            Children = { label, helpLine, entry, _result },
        };
        ShowShell();
    }

    /// <summary>
    /// Raised with the rules that matched, from 0, after each test; empty when none did.
    /// </summary>
    public event Action<IReadOnlySet<int>>? Matched;

    public FrameworkElement Root { get; }

    /// <summary>
    /// Tests the line again after a short pause, such as after the rules changed.
    /// </summary>
    public void Retest()
    {
        _debounce.Stop();
        _debounce.Start();
    }

    /// <summary>
    /// Shows the shared shell choice, which the other box may have changed.
    /// </summary>
    public void ShowShell() => _shellBox.SelectedIndex = _shell.Value == Shell.Powershell ? 1 : 0;

    private async Task RunAsync()
    {
        var line = _line.Text;
        var request = ++_request;
        if (line.Trim().Length == 0)
        {
            Show(string.Empty, null);
            return;
        }

        var session = _context.Session;
        if (session.Status != SessionStatus.Ready)
        {
            Show(LiveText.ServiceStopped, null);
            return;
        }

        if (_scope() is not { } scope)
        {
            Show(RulesText.DraftNotReady, null);
            return;
        }

        var reply = await session.ClassifyAsync(line, _shell.Value, scope.Changes)
            .ConfigureAwait(true);
        if (request != _request)
        {
            return;
        }

        if (reply.Value is not { } test)
        {
            Show($"Can't test this now: {reply.Failure?.Message}", null);
            return;
        }

        var classes = (session.Setting("classes") as SizeTableModel)?.Control;
        var lines = RulesText.TestLines(
            test,
            scope.Rules,
            RulesText.ClassLabels(classes),
            _context.Number);
        Show(string.Join("\n", lines), test);
    }

    // The test's result is the outcome of the user's typing, so it is spoken (M7.1).
    private void Show(string text, CommandTest? test)
    {
        if (SettingsUi.SetLive(_result, text) && text.Length > 0)
        {
            SettingsUi.Announce(_result, text);
        }

        Matched?.Invoke(RulesText.Matched(test));
    }
}
