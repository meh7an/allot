using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// The Edit dialog of a heavy-command rule, and Add rule (DESIGN.md section 4, Commands): program,
/// arguments as chips and the lease class, with a test box that tries the rule before Save. Save
/// writes the whole list at once.
/// </summary>
internal static class RuleDialog
{
    /// <summary>
    /// Shows the dialog for <paramref name="draft"/>.
    /// </summary>
    /// <returns>Whether the rule was handed over to be written.</returns>
    public static async Task<bool> ShowAsync(
        PageContext context,
        RuleListModel rules,
        RuleDraft draft,
        ShellChoice shell)
    {
        var syncs = new List<Action>();
        RowNote? Note(ValueIssue? issue) =>
            issue is null
                ? null
                : new RowNote(RowNoteKind.Error, ValueText.IssueText(issue, context.Number));

        var program = new TextBox
        {
            Text = draft.Program,
            Width = 240,
            FontFamily = SettingsUi.Mono,
            IsSpellCheckEnabled = false,
        };
        program.TextChanged += (_, _) => draft.SetProgram(program.Text);
        var programRow = new SettingRow(
            draft.ProgramSpec.Label,
            draft.ProgramSpec.Help,
            null,
            program);
        syncs.Add(() => programRow.SetNote(Note(draft.ProgramIssue)));

        var argList = new ChipsEditor(draft.Args, "New argument pattern", "build", "Add argument");
        argList.Root.MaxWidth = 300;
        var argsSpec = draft.Args.Spec;
        var argsRow = new SettingRow(argsSpec.Label, argsSpec.Help, null, argList.Root);
        syncs.Add(() =>
        {
            argList.Sync(true);
            argsRow.SetNote(Note(draft.Args.Issue));
        });

        var classBox = new ComboBox { MinWidth = 160 };
        var options = draft.ClassOptions;
        foreach (var option in options)
        {
            classBox.Items.Add(option.Label);
        }

        classBox.SelectedIndex = IndexOf(options, draft.RuleClass);
        classBox.SelectionChanged += (_, _) =>
        {
            if (classBox.SelectedIndex >= 0)
            {
                draft.SetClass(options[classBox.SelectedIndex].Value);
            }
        };
        var classRow = new SettingRow(draft.ClassSpec.Label, draft.ClassSpec.Help, null, classBox);
        syncs.Add(() => classRow.SetNote(Note(draft.ClassIssue)));

        var test = new CommandTestBox(
            context,
            "Shows how the hook would treat this command with this rule saved.",
            () => Scope(rules, draft),
            shell);
        test.Root.Margin = new Thickness(0, 12, 0, 0);

        void Sync()
        {
            foreach (var sync in syncs)
            {
                sync();
            }
        }

        draft.PropertyChanged += (_, _) =>
        {
            Sync();
            test.Retest();
        };
        Sync();

        var dialog = new ContentDialog
        {
            Title = RulesText.DialogTitle(draft),
            Content = new StackPanel
            {
                Spacing = 4,
                Children = { programRow, argsRow, classRow, test.Root },
            },
            PrimaryButtonText = "Save",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.None,
        };
        dialog.Resources["ContentDialogMaxWidth"] = 720.0;
        dialog.PrimaryButtonClick += (_, args) => args.Cancel = !rules.Save(draft);
        var result = await context.Dialog(dialog).ConfigureAwait(true);
        return result == ContentDialogResult.Primary;
    }

    private static TestScope? Scope(RuleListModel rules, RuleDraft draft)
    {
        if (rules.Proposal(draft) is not { } changes)
        {
            return null;
        }

        var proposed = changes.OfType<Change.SetRules>().Single().Rules;
        return new TestScope(changes, proposed);
    }

    private static int IndexOf(IReadOnlyList<ChoiceOption> options, string? value)
    {
        for (var i = 0; i < options.Count; i++)
        {
            if (options[i].Value == value)
            {
                return i;
            }
        }

        return -1;
    }
}
