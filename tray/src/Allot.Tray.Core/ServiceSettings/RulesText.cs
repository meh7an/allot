using System.Globalization;

using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The Commands page's sentences: the rule list's status and rows, its confirmations and toasts,
/// and the lines of Test a command (DESIGN.md section 4, Commands).
/// </summary>
public static class RulesText
{
    public const string BuiltIn = "Built-in rules.";

    public const string Yours = "Your rules. They replace the built-in list.";

    public const string Empty =
        "No rules. No command counts as heavy, so the hook never asks for a lease.";

    public const string AnyArguments = "any arguments";

    public const string NoCommand = "No command to test in this line.";

    public const string DraftNotReady = "Enter a program to test this rule.";

    /// <summary>
    /// The status line above the list.
    /// </summary>
    public static string Status(RuleListModel rules) => rules.CanReset ? Yours : BuiltIn;

    /// <summary>
    /// A rule's arguments in its row: "build · test · run", or "any arguments".
    /// </summary>
    public static string Arguments(HeavyRule rule) =>
        rule.Args.Count == 0 ? AnyArguments : string.Join(" · ", rule.Args);

    /// <summary>
    /// A rule's row as a screen reader reads it: "Rule 1: cargo, build · test, CPU heavy".
    /// </summary>
    public static string RowName(
        int index,
        HeavyRule rule,
        Func<JobClass, string> classLabel) =>
        $"Rule {Ordinal(index)}: {rule.Program}, {Arguments(rule)}, {classLabel(rule.Class)}";

    /// <summary>
    /// The accessible name of a rule's menu button.
    /// </summary>
    public static string MoreOptionsName(int index) => $"More options for rule {Ordinal(index)}";

    /// <summary>
    /// The toast after a delete, with Undo.
    /// </summary>
    public static string Deleted(int index) => $"Rule {Ordinal(index)} deleted.";

    /// <summary>
    /// The rule dialog's title.
    /// </summary>
    public static string DialogTitle(RuleDraft draft) =>
        draft.Index is { } index ? $"Edit rule {Ordinal(index)}" : "Add rule";

    /// <summary>
    /// Use built-in rules' confirmation.
    /// </summary>
    /// <param name="current">The rules in the list now.</param>
    /// <param name="builtIn">The built-in rules.</param>
    public static Confirmation UseBuiltIn(int current, int builtIn)
    {
        var yours = current == 1 ? "Your 1 rule is" : $"Your {Count(current)} rules are";
        var ones = builtIn == 1 ? "built-in one" : $"{Count(builtIn)} built-in ones";
        return new Confirmation(
            "Use the built-in rules?",
            $"{yours} replaced by the {ones}. This can't be undone.",
            "Use built-in rules");
    }

    /// <summary>
    /// The label of a class from the Classes table's spec, such as "CPU heavy".
    /// </summary>
    public static Func<JobClass, string> ClassLabels(SettingControl.SizeTable? classes) =>
        jobClass =>
        {
            var name = ClassNames.Of(jobClass);
            return classes?.Rows.FirstOrDefault(row => row.Name == name)?.Label ?? name;
        };

    /// <summary>
    /// One line per simple command of the tested line, and one more naming the combined lease
    /// when more than one command is heavy.
    /// </summary>
    /// <param name="test">allotd's answer.</param>
    /// <param name="rules">The rules it was tested against, for the matched rule's text.</param>
    /// <param name="classLabel">Names a class, such as "CPU heavy".</param>
    /// <param name="number">Formats a number for the user's locale.</param>
    public static IReadOnlyList<string> TestLines(
        CommandTest test,
        IReadOnlyList<HeavyRule> rules,
        Func<JobClass, string> classLabel,
        Func<double, string> number)
    {
        if (test.Commands.Count == 0)
        {
            return [NoCommand];
        }

        var lines = new List<string>();
        foreach (var result in test.Commands)
        {
            lines.Add(result.Kind switch
            {
                CommandKind.Heavy => HeavyLine(result, rules, classLabel, number),
                CommandKind.PrintOnly =>
                    $"Print-only: {result.Flag} only prints and exits, so no lease is needed.",
                _ => $"Not heavy: no rule matches {result.Command}.",
            });
        }

        var heavy = test.Commands.Count(result => result.Kind == CommandKind.Heavy);
        if (heavy > 1 && test.Lease is { } lease)
        {
            lines.Add($"The whole line needs one {Lease(lease, classLabel, number)}.");
        }

        return lines;
    }

    /// <summary>
    /// The rules that matched a command of the tested line, from 0, for the "Matches" mark.
    /// </summary>
    public static IReadOnlySet<int> Matched(CommandTest? test)
    {
        var matched = new HashSet<int>();
        if (test is null)
        {
            return matched;
        }

        foreach (var result in test.Commands)
        {
            if (result.Kind == CommandKind.Heavy && result.RuleIndex is { } index)
            {
                _ = matched.Add((int)index);
            }
        }

        return matched;
    }

    private static string HeavyLine(
        CommandResult result,
        IReadOnlyList<HeavyRule> rules,
        Func<JobClass, string> classLabel,
        Func<double, string> number)
    {
        var needs = result.Size is { } size
            ? $"Heavy: needs a {Lease(size, classLabel, number)}."
            : "Heavy.";
        if (result.RuleIndex is not { } index || index >= rules.Count)
        {
            return needs;
        }

        var rule = rules[(int)index];
        var args = rule.Args.Count == 0 ? AnyArguments : Alternatives(rule.Args);
        return $"{needs} Rule {Ordinal((int)index)}: {rule.Program} with {args}.";
    }

    // "CPU heavy lease (8 cores, 4 GB memory)"; the GPU parts only when the class asks for them.
    private static string Lease(
        ClassSize size,
        Func<JobClass, string> classLabel,
        Func<double, string> number)
    {
        var profile = size.Profile;
        string Of(double value, Quantity quantity) =>
            ValueText.QuantityText(value, quantity, number);
        var parts = new List<string>
        {
            Of(profile.Cores, Quantity.Cores),
            $"{Of(profile.RamMb, Quantity.Megabytes)} memory",
        };
        if (profile.GpuPct > 0)
        {
            parts.Add($"{Of(profile.GpuPct, Quantity.Percent)} of the GPU");
        }

        if (profile.VramMb > 0)
        {
            parts.Add($"{Of(profile.VramMb, Quantity.Megabytes)} video memory");
        }

        return $"{classLabel(size.Class)} lease ({string.Join(", ", parts)})";
    }

    // "build, test, run or bench".
    private static string Alternatives(IReadOnlyList<string> parts) =>
        parts.Count < 2
            ? string.Concat(parts)
            : $"{string.Join(", ", parts.Take(parts.Count - 1))} or {parts[^1]}";

    private static string Ordinal(int index) => Count(index + 1);

    private static string Count(int value) => value.ToString(CultureInfo.InvariantCulture);
}
