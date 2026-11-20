using System.Globalization;

using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The sentences of the Settings window's values: the line under a field with an issue, and a
/// value with its unit.
/// </summary>
public static class ValueText
{
    /// <summary>
    /// The sentence under a field for <paramref name="issue"/>.
    /// </summary>
    /// <param name="issue">What is wrong.</param>
    /// <param name="number">Formats a number for the user's locale.</param>
    public static string IssueText(ValueIssue issue, Func<double, string> number) =>
        issue.Kind switch
        {
            IssueKind.Required => "Enter a value.",
            IssueKind.NotANumber => "Enter a number.",
            IssueKind.Whole => "Enter a whole number.",
            IssueKind.Range => $"Enter a number {RangeText(issue.Min, issue.Max, number)}.",
            IssueKind.AtLeast => Compared("at least", issue, number),
            IssueKind.AtMost => Compared("at most", issue, number),
            IssueKind.Blank => "Enter some text.",
            IssueKind.Pattern => "Use only the characters the help names.",
            IssueKind.Duplicate => "The list already has this.",
            IssueKind.NameTaken => "Another preset or a class already has this name.",
            IssueKind.Service => issue.Message ?? string.Empty,
            _ => throw new ArgumentOutOfRangeException(nameof(issue), issue.Kind, null),
        };

    /// <summary>
    /// A value with its unit, such as "15 %", "3 minutes" or "2 GB". Durations take the largest
    /// unit that divides them, and memory GB when it is a whole number of them.
    /// </summary>
    /// <param name="value">In the setting's own unit.</param>
    /// <param name="quantity">What the value counts.</param>
    /// <param name="number">Formats a number for the user's locale.</param>
    public static string QuantityText(
        double value,
        Quantity quantity,
        Func<double, string> number)
    {
        switch (quantity)
        {
            case Quantity.Percent:
                return $"{number(value)} %";
            case Quantity.Runs:
                return Plural(value, "run", number);
            case Quantity.Days:
                return Plural(value, "day", number);
            case Quantity.Cores:
                return Plural(value, "core", number);
            case Quantity.Seconds:
                {
                    var unit = ValueUnit.Best(ValueUnit.Durations, value);
                    return Plural(value / unit.Factor, unit.Singular, number);
                }

            case Quantity.Megabytes:
                {
                    var unit = ValueUnit.Best(ValueUnit.Memory, value);
                    return $"{number(value / unit.Factor)} {unit.Name}";
                }

            case Quantity.Port:
                return value.ToString(CultureInfo.InvariantCulture);
            default:
                throw new ArgumentOutOfRangeException(nameof(quantity), quantity, null);
        }
    }

    private static string Compared(string bound, ValueIssue issue, Func<double, string> number)
    {
        var value = QuantityText(issue.Value, issue.Quantity, number);
        return $"Must be {bound} {issue.Label} ({value}).";
    }

    // "from 0 to 99", "at least 1", "above 0", "at least 0 and below 100".
    private static string RangeText(Bound? min, Bound? max, Func<double, string> number)
    {
        if (min is { Exclusive: false } low && max is { Exclusive: false } high)
        {
            return $"from {number(low.Value)} to {number(high.Value)}";
        }

        var parts = new List<string>();
        if (min is not null)
        {
            var bound = min.Exclusive ? "above" : "at least";
            parts.Add($"{bound} {number(min.Value)}");
        }

        if (max is not null)
        {
            var bound = max.Exclusive ? "below" : "at most";
            parts.Add($"{bound} {number(max.Value)}");
        }

        return string.Join(" and ", parts);
    }

    private static string Plural(double value, string noun, Func<double, string> number)
    {
        var word = value == 1 ? noun : noun + "s";
        return $"{number(value)} {word}";
    }
}
