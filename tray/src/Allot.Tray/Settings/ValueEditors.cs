using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ServiceSettings;

using Microsoft.UI.Xaml.Controls;

namespace Allot.Tray.Settings;

/// <summary>
/// What every editor on a page shares: the session, the number format and the Undo toast.
/// </summary>
/// <param name="Session">The settings session.</param>
/// <param name="Number">Formats a number for the user's locale.</param>
/// <param name="Toast">Shows a brief notice, with an Undo action or none.</param>
/// <param name="Confirm">Asks in a dialog; true when the user chose the primary button.</param>
/// <param name="Dialog">Shows a dialog, such as a row's Edit dialog; one at a time, and hiding
/// the window cancels it. <c>None</c> when another dialog is open.</param>
internal sealed record PageContext(
    SettingsSession Session,
    Func<double, string> Number,
    Action<string, Action?> Toast,
    Func<Confirmation, Task<bool>> Confirm,
    Func<ContentDialog, Task<ContentDialogResult>> Dialog);

/// <summary>
/// The rows of one allotd setting, each control kept in step with its view model (DESIGN.md
/// section 3). A number commits on Enter, on leaving the field or on a spin step; Escape shows the
/// saved value again.
/// </summary>
internal static class ValueEditors
{
    /// <summary>
    /// The rows of <paramref name="model"/>: one, or two for a duration with an off switch.
    /// </summary>
    public static IReadOnlyList<SettingRow> For(ValueModel model, PageContext context) =>
        model switch
        {
            SwitchModel switchModel => [SwitchRow(switchModel, context)],
            DurationModel { Off: { } off } duration => OffDurationRows(duration, off, context),
            NumberModel number => [NumberRow(number, context, model.Key)],
            StringListModel list => [ChipsRow(list, context)],
            _ => [],
        };

    // The pages' one string list is the print-only flags (DESIGN.md, Commands). The chips sit
    // beside the label like any control; the width bound lets them wrap.
    private static SettingRow ChipsRow(StringListModel model, PageContext context)
    {
        var editor = new ChipsEditor(model, "New print-only flag", "--dry-run", "Add flag");
        editor.Root.MaxWidth = 420;
        var row = new SettingRow(
            model.Label,
            model.Spec.Help,
            model.Key,
            editor.Root,
            editor.Field);
        Follow(model, context, () =>
        {
            editor.Sync(Editable(model, context));
            ShowState(row, model, context);
        });
        return row;
    }

    private static SettingRow SwitchRow(SwitchModel model, PageContext context)
    {
        var toggle = new ToggleSwitch();
        var row = new SettingRow(model.Label, model.Spec.Help, model.Key, toggle);
        var syncing = false;
        toggle.Toggled += (_, _) =>
        {
            if (!syncing && toggle.IsOn != model.Value)
            {
                model.Turn(toggle.IsOn);
            }
        };
        Follow(model, context, () =>
        {
            syncing = true;
            toggle.IsOn = model.Value;
            syncing = false;
            toggle.IsEnabled = Editable(model, context);
            ShowState(row, model, context);
        });
        return row;
    }

    // "End leases left idle" with the duration indented under it; the switch row holds the key
    // and the reset button, the duration row the line under the value.
    private static List<SettingRow> OffDurationRows(
        DurationModel model,
        OffSwitch off,
        PageContext context)
    {
        var toggle = new ToggleSwitch();
        var switchRow = new SettingRow(off.Label, off.Help, model.Key, toggle);
        var editor = new NumberEditor(model);
        var durationRow = new SettingRow(
            model.Label,
            model.Spec.Help,
            null,
            editor.Root,
            editor.Field)
        {
            Indented = true,
        };
        var syncing = false;
        toggle.Toggled += (_, _) =>
        {
            if (!syncing && toggle.IsOn != model.IsOn)
            {
                model.SetOn(toggle.IsOn);
            }
        };
        Follow(model, context, () =>
        {
            syncing = true;
            toggle.IsOn = model.IsOn;
            syncing = false;
            toggle.IsEnabled = Editable(model, context);
            editor.Sync(Editable(model, context) && model.IsOn);
            switchRow.SetReset(
                SettingsText.ResetButtonName(model, context.Number),
                ResetAction(model, context));
            durationRow.SetNote(Note(model, context));
        });
        return [switchRow, durationRow];
    }

    private static SettingRow NumberRow(NumberModel model, PageContext context, string key)
    {
        var editor = new NumberEditor(model);
        var row = new SettingRow(model.Label, model.Spec.Help, key, editor.Root, editor.Field);
        Follow(model, context, () =>
        {
            editor.Sync(Editable(model, context));
            ShowState(row, model, context);
        });
        return row;
    }

    // The model changes on edits and refreshes; the session on everything else that bears on a row
    // (read-only states, a switch another setting requires, a restart).
    private static void Follow(ValueModel model, PageContext context, Action sync)
    {
        model.PropertyChanged += (_, _) => sync();
        context.Session.PropertyChanged += (_, _) => sync();
        sync();
    }

    private static bool Editable(ValueModel model, PageContext context) =>
        !context.Session.ReadOnly && model.Enabled;

    private static void ShowState(SettingRow row, ValueModel model, PageContext context)
    {
        var name = SettingsText.ResetButtonName(model, context.Number);
        row.SetReset(name, ResetAction(model, context));
        row.SetNote(Note(model, context));
    }

    private static Action? ResetAction(ValueModel model, PageContext context)
    {
        if (!model.CanReset || context.Session.ReadOnly)
        {
            return null;
        }

        return () =>
        {
            var undo = FieldReset.Reset(model);
            context.Toast(SettingsText.FieldReset(model, context.Number), undo);
        };
    }

    private static RowNote? Note(ValueModel model, PageContext context)
    {
        if (model.Issue is { } issue)
        {
            var undo = issue.Kind == IssueKind.Service ? model.Undo : (Action?)null;
            return new RowNote(RowNoteKind.Error, ValueText.IssueText(issue, context.Number), undo);
        }

        if (model.DisabledNote is { } disabled)
        {
            return new RowNote(RowNoteKind.Info, disabled);
        }

        return SettingsBanners.RestartNote(context.Session, model.Key) is { } restart
            ? new RowNote(RowNoteKind.Caution, restart)
            : null;
    }
}
