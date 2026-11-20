using System.Globalization;
using System.Text.RegularExpressions;

using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The heavy-command rules: an ordered list edited one rule at a time in a dialog, reordered and
/// deleted from each row's menu, and written whole on every change.
/// </summary>
public sealed partial class RuleListModel : ValueModel<IReadOnlyList<HeavyRule>>
{
    public RuleListModel(SettingSpec spec, IValueHost host, IReadOnlyList<HeavyRule> value)
        : base(spec, host, value)
    {
        Control = spec.Control as SettingControl.RuleList
            ?? throw new ArgumentException($"{spec.Key} is not a rule list", nameof(spec));
    }

    public SettingControl.RuleList Control { get; }

    /// <inheritdoc/>
    protected override bool HasDefault => true;

    /// <inheritdoc/>
    protected override IReadOnlyList<HeavyRule> DefaultValue => Control.Default;

    /// <summary>
    /// Names the rule, from 1, and its field for <c>classifier.rules[&lt;i&gt;].&lt;field&gt;</c>.
    /// </summary>
    public override string DescribeProblem(string problemKey, string message)
    {
        var rest = problemKey[Math.Min(Key.Length, problemKey.Length)..];
        var match = RuleProblemKey().Match(rest);
        if (!match.Success)
        {
            return base.DescribeProblem(problemKey, message);
        }

        var fieldKey = match.Groups[2].Value;
        var field = Control.Fields.FirstOrDefault(candidate => candidate.Key == fieldKey);
        var subject = field is null ? string.Empty : $" {field.Label}";
        var number = int.Parse(match.Groups[1].Value, CultureInfo.InvariantCulture) + 1;
        return $"Rule {number.ToString(CultureInfo.InvariantCulture)}:{subject} {message}";
    }

    /// <summary>
    /// A draft of the rule at <paramref name="index"/> for its edit dialog.
    /// </summary>
    public RuleDraft? Edit(int index) =>
        index >= 0 && index < Value.Count
            ? new RuleDraft(Control.Fields, Value[index], index)
            : null;

    /// <summary>
    /// A draft of a new rule, added at the end.
    /// </summary>
    public RuleDraft Add() => new(Control.Fields, null, null);

    /// <summary>
    /// Writes the list with the draft's rule in its place.
    /// </summary>
    /// <returns>Whether it was handed over to be written.</returns>
    public bool Save(RuleDraft draft)
    {
        if (!draft.Check())
        {
            return false;
        }

        var rules = new List<HeavyRule>(Value);
        if (draft.Index is { } index)
        {
            rules[index] = draft.ToRule();
        }
        else
        {
            rules.Add(draft.ToRule());
        }

        Write(rules);
        return true;
    }

    /// <summary>
    /// Moves a rule; rules match in list order.
    /// </summary>
    public void Move(int fromIndex, int toIndex)
    {
        if (fromIndex < 0 || fromIndex >= Value.Count)
        {
            return;
        }

        var rules = new List<HeavyRule>(Value);
        var rule = rules[fromIndex];
        rules.RemoveAt(fromIndex);
        if (toIndex < 0 || toIndex > rules.Count)
        {
            return;
        }

        rules.Insert(toIndex, rule);
        Write(rules);
    }

    public void Remove(int index)
    {
        if (index >= 0 && index < Value.Count)
        {
            Write([.. Value.Where((_, at) => at != index)]);
        }
    }

    /// <inheritdoc/>
    public override bool Take(SettingValue value)
    {
        if (value is not SettingValue.RulesValue rulesValue)
        {
            return false;
        }

        Load(rulesValue.Value);
        return true;
    }

    /// <inheritdoc/>
    protected override bool Same(IReadOnlyList<HeavyRule> left, IReadOnlyList<HeavyRule> right) =>
        left.Count == right.Count
        && left.Zip(right).All(pair => SameRule(pair.First, pair.Second));

    [GeneratedRegex(@"^\[([0-9]+)\](?:\.([A-Za-z0-9_]+))?$", RegexOptions.CultureInvariant)]
    private static partial Regex RuleProblemKey();

    private static bool SameRule(HeavyRule left, HeavyRule right) =>
        left.Program == right.Program
        && left.Class == right.Class
        && left.Args.SequenceEqual(right.Args);

    private void Write(IReadOnlyList<HeavyRule> rules) =>
        Apply(rules, null, new Change.SetRules { Rules = rules }, true);
}
