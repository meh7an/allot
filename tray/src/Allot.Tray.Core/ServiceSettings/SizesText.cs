using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The cells of a class or preset row in the Sizes tables, each with its unit.
/// </summary>
/// <param name="Cores">"8".</param>
/// <param name="MinCores">"4", or "Automatic (4)" while the row names none.</param>
/// <param name="Memory">"4 GB".</param>
/// <param name="Gpu">"50 %".</param>
/// <param name="VideoMemory">"0 MB".</param>
/// <param name="RunTime">"5 minutes".</param>
public sealed record SizeCells(
    string Cores,
    string MinCores,
    string Memory,
    string Gpu,
    string VideoMemory,
    string RunTime);

/// <summary>
/// The Sizes page's sentences: table cells, the Fits now column, the edit dialog's title and live
/// line, and the row actions' names, toasts and confirmations (DESIGN.md section 4, Sizes).
/// </summary>
public static class SizesText
{
    /// <summary>
    /// The Fits now cell while no preview is known, such as while the service is stopped.
    /// </summary>
    public const string FitsUnknown = "—";

    public const string AddPresetTitle = "Add preset";

    /// <summary>
    /// The fewest cores a row that names no minimum starts with: half the cores, at least one,
    /// or all of them when there are fewer (allotd's <c>default_min_cores</c>).
    /// </summary>
    public static double AutomaticMinCores(double cores) => Math.Max(cores / 2, Math.Min(cores, 1));

    public static SizeCells Cells(SizeRow row, Func<double, string> number)
    {
        var profile = row.Profile;
        var minimum = row.MinCores is { } custom
            ? number(custom)
            : $"Automatic ({number(AutomaticMinCores(profile.Cores))})";
        return new SizeCells(
            number(profile.Cores),
            minimum,
            ValueText.QuantityText(profile.RamMb, Quantity.Megabytes, number),
            ValueText.QuantityText(profile.GpuPct, Quantity.Percent, number),
            ValueText.QuantityText(profile.VramMb, Quantity.Megabytes, number),
            ValueText.QuantityText(profile.EstSecs, Quantity.Seconds, number));
    }

    /// <summary>
    /// The Fits now cell: "1, 2 more shrunk", "2 shrunk", "3", "at least 99" or "None".
    /// </summary>
    /// <param name="fit">The row's fit preview, or <c>null</c> while none is known.</param>
    /// <param name="number">Formats a number for the user's locale.</param>
    public static string FitsNow(SizeFit? fit, Func<double, string> number)
    {
        if (fit is null)
        {
            return FitsUnknown;
        }

        var fits = fit.Now;
        if (fits.Full == 0 && fits.Shrunk == 0)
        {
            return "None";
        }

        // Counts stop at allotd's limit; the larger count then reads "at least 99".
        var capped = fits.More ? "at least " : string.Empty;
        if (fits.Shrunk == 0)
        {
            return $"{capped}{number(fits.Full)}";
        }

        var shrunk = $"{capped}{number(fits.Shrunk)}";
        return fits.Full == 0
            ? $"{shrunk} shrunk"
            : $"{number(fits.Full)}, {shrunk} more shrunk";
    }

    /// <summary>
    /// The edit dialog's live line: "With these values, now: 1 at full size, 2 more at 4 cores.
    /// With every lease ended: 2 at full size."
    /// </summary>
    public static string DialogFits(SizeFit fit, Func<double, string> number)
    {
        var now = Lowered(LiveText.Fits(fit.Now, fit.MinCores, number));
        var idle = Lowered(LiveText.Fits(fit.Idle, fit.MinCores, number));
        return $"With these values, now: {now}. With every lease ended: {idle}.";
    }

    /// <summary>
    /// The edit dialog's title: "Edit CPU heavy", "Edit preset cpu_small" or "Add preset".
    /// </summary>
    public static string DialogTitle(SizeTableModel table, RowDraft draft)
    {
        if (draft.Original is not { } name)
        {
            return AddPresetTitle;
        }

        return table.CanAdd ? $"Edit preset {name}" : $"Edit {RowLabel(table, name)}";
    }

    /// <summary>
    /// The dialog's warning while a preset is renamed; <c>null</c> otherwise.
    /// </summary>
    public static string? RenameWarning(RowDraft draft) =>
        draft.Original is { } original
        && draft.CanRename
        && draft.Name.Length > 0
        && draft.Name != original
            ? $"Sessions that ask for {original} get an error until they use {draft.Name}."
            : null;

    /// <summary>
    /// The accessible name of a row's Edit button: "Edit CPU heavy", "Edit preset cpu_small".
    /// </summary>
    public static string EditName(SizeTableModel table, SizeRowView row) =>
        $"Edit {Subject(table, row)}";

    /// <summary>
    /// The accessible name of a row's reset button: "Reset CPU heavy to its built-in size".
    /// </summary>
    public static string ResetName(SizeTableModel table, SizeRowView row) =>
        $"Reset {Subject(table, row)} to its built-in size";

    /// <summary>
    /// The accessible name of a preset's delete button: "Delete preset render".
    /// </summary>
    public static string DeleteName(SizeRowView row) => $"Delete preset {row.Row.Name}";

    /// <summary>
    /// The toast after a row's reset or a preset's delete, both with Undo.
    /// </summary>
    public static string RowRemoved(SizeTableModel table, SizeRowView row) =>
        row.BuiltIn
            ? $"{Capitalized(Subject(table, row))} reset to its built-in size."
            : $"Preset {row.Row.Name} deleted.";

    public static Confirmation DeletePreset(string name) =>
        new(
            $"Delete preset {name}?",
            "Sessions that ask for it by name get an error. You can add it again later.",
            "Delete preset");

    // "CPU heavy" for a class, "preset cpu_small" for a preset.
    private static string Subject(SizeTableModel table, SizeRowView row) =>
        table.CanAdd ? $"preset {row.Row.Name}" : row.Label;

    private static string RowLabel(SizeTableModel table, string name) =>
        table.Rows.FirstOrDefault(row => row.Row.Name == name)?.Label ?? name;

    private static string Capitalized(string text) =>
        text.Length == 0 ? text : char.ToUpperInvariant(text[0]) + text[1..];

    // "None fit" reads lowercase inside a sentence.
    private static string Lowered(string fits) =>
        fits == LiveText.NoneFit ? "none fit" : fits;
}
