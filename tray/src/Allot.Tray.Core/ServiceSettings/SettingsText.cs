using System.Globalization;

using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The Settings window's fixed sentences: the save status, the reset confirmations and toasts, and
/// a saved or pending value as the user reads it.
/// </summary>
public static class SettingsText
{
    public const string NothingToReset = "Nothing to reset";

    /// <summary>
    /// A number for <paramref name="culture"/>, grouped, with at most two decimals.
    /// </summary>
    public static Func<double, string> Numbers(CultureInfo culture) =>
        value => value.ToString("#,0.##", culture);

    /// <summary>
    /// "All changes saved", "Saving…", "1 change not saved".
    /// </summary>
    public static string SaveStatus(SaveStatus status) => status.Kind switch
    {
        SaveKind.Saved => "All changes saved",
        SaveKind.Saving => "Saving…",
        _ => status.Count == 1 ? "1 change not saved" : $"{status.Count} changes not saved",
    };

    /// <summary>
    /// The default of a setting with its unit, such as "15 %" or "Off"; empty without one.
    /// </summary>
    public static string DefaultOf(ValueModel model, Func<double, string> number) =>
        model.Spec.Control switch
        {
            SettingControl.Switch control => OnOff(control.Default),
            SettingControl.Number { Off: { } off } control when control.Default == off.Value =>
                "Off",
            SettingControl.Number { Default: { } value } control =>
                ValueText.QuantityText(value, control.Quantity, number),
            SettingControl.StringList control => string.Join(", ", control.Default),
            _ => string.Empty,
        };

    /// <summary>
    /// The value a change would write, for the conflict banner: "15 %", "On", "the default".
    /// </summary>
    public static string ChangeValue(ValueModel model, Change change, Func<double, string> number) =>
        change switch
        {
            Change.SetSwitch set => OnOff(set.Value),
            Change.SetNumber set when model is DurationModel { Off: { } off }
                && set.Value == off.Value => "Off",
            Change.SetNumber set when model is NumberModel numberModel =>
                ValueText.QuantityText(set.Value, numberModel.Quantity, number),
            Change.SetList set => string.Join(", ", set.Value),
            _ => "the default",
        };

    /// <summary>
    /// The toast after a field's reset: "Headroom reset to 15 %."
    /// </summary>
    public static string FieldReset(ValueModel model, Func<double, string> number) =>
        DefaultOf(model, number) is { Length: > 0 } value
            ? $"{model.Label} reset to {value}."
            : $"{model.Label} reset.";

    /// <summary>
    /// The accessible name of a field's reset button: "Reset Headroom to 15 %".
    /// </summary>
    public static string ResetButtonName(ValueModel model, Func<double, string> number) =>
        DefaultOf(model, number) is { Length: > 0 } value
            ? $"Reset {model.Label} to {value}"
            : $"Reset {model.Label}";

    /// <summary>
    /// Reset page's confirmation: title, body and button.
    /// </summary>
    /// <param name="pageLabel">The page, such as Admission.</param>
    /// <param name="changed">The settings on it that differ from their default.</param>
    /// <param name="number">Formats a number for the user's locale.</param>
    public static Confirmation ResetPage(
        string pageLabel,
        IReadOnlyList<ValueModel> changed,
        Func<double, string> number) =>
        ResetPage(
            pageLabel,
            [
                .. changed.Select(model => DefaultOf(model, number) is { Length: > 0 } value
                    ? $"{model.Label}: {value}"
                    : model.Label),
            ]);

    /// <summary>
    /// Reset page's confirmation, from one line per changed setting with its default.
    /// </summary>
    public static Confirmation ResetPage(string pageLabel, IReadOnlyList<string> lines)
    {
        var count = Settings(lines.Count);
        return new Confirmation(
            $"Reset {count} on {pageLabel}?",
            $"These go back to their defaults:\n{string.Join("\n", lines)}",
            $"Reset {count}");
    }

    /// <summary>
    /// Reset all service settings' confirmation.
    /// </summary>
    public static Confirmation ResetAll(int changed)
    {
        var count = Settings(changed);
        return new Confirmation(
            changed == 1 ? "Reset the 1 changed setting?" : $"Reset all {changed} changed settings?",
            "config.toml goes back to the defaults. Your rules and presets are replaced by the "
                + "built-in ones, and presets you added are deleted. Comments next to these keys "
                + "are removed too. This can't be undone.",
            $"Reset {count}");
    }

    public static string OnOff(bool on) => on ? "On" : "Off";

    /// <summary>
    /// "a", "a and b", "a, b and c".
    /// </summary>
    internal static string Joined(IReadOnlyList<string> parts) =>
        parts.Count < 2
            ? string.Concat(parts)
            : $"{string.Join(", ", parts.Take(parts.Count - 1))} and {parts[^1]}";

    private static string Settings(int count) => count == 1 ? "1 setting" : $"{count} settings";
}

/// <summary>
/// A confirmation dialog, such as Reset page or Stop service.
/// </summary>
/// <param name="Title">The dialog's title.</param>
/// <param name="Body">What happens, such as what goes back to its default.</param>
/// <param name="Button">The primary button.</param>
public sealed record Confirmation(string Title, string Body, string Button);
