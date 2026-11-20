using System.Text.RegularExpressions;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The fields of one heavy-command rule in its edit dialog, kept until Save. It is the
/// <see cref="IValueHost"/> of its argument list.
/// </summary>
public sealed class RuleDraft : Observable, IValueHost
{
    private readonly IReadOnlyList<SettingSpec> _fields;
    private string _program;
    private ValueIssue? _programIssue;
    private string? _ruleClass;
    private ValueIssue? _classIssue;

    internal RuleDraft(IReadOnlyList<SettingSpec> fields, HeavyRule? rule, int? index)
    {
        _fields = fields;
        Index = index;
        _program = rule?.Program ?? string.Empty;
        _ruleClass = rule is null ? Choice()?.Default : ClassName(rule.Class);
        Args = new StringListModel(Field("args"), this, rule?.Args ?? []);
    }

    /// <summary>
    /// The program as typed, trimmed.
    /// </summary>
    public string Program => _program;

    public ValueIssue? ProgramIssue => _programIssue;

    /// <summary>
    /// The class the rule gives a matched command, as the file names it, such as
    /// <c>cpu_heavy</c>; <c>null</c> until one is picked.
    /// </summary>
    public string? RuleClass => _ruleClass;

    public ValueIssue? ClassIssue => _classIssue;

    /// <summary>
    /// The argument patterns, as chips; they are saved with the rule, not on their own.
    /// </summary>
    public StringListModel Args { get; }

    /// <summary>
    /// The rule's place in the list, or <c>null</c> for a new rule.
    /// </summary>
    public int? Index { get; }

    /// <summary>
    /// The program field's spec.
    /// </summary>
    public SettingSpec ProgramSpec => Field("program");

    /// <summary>
    /// The class field's spec.
    /// </summary>
    public SettingSpec ClassSpec => Field("class");

    public IReadOnlyList<ChoiceOption> ClassOptions => Choice()?.Options ?? [];

    public void SetProgram(string text)
    {
        _program = text.Trim();
        _programIssue = FindProgramIssue();
        Raise(null);
    }

    /// <param name="value">The class as the file names it, one of <see cref="ClassOptions"/>.
    /// </param>
    public void SetClass(string value)
    {
        _ruleClass = value;
        _classIssue = null;
        Raise(null);
    }

    /// <returns>Whether the rule can be saved.</returns>
    public bool Check()
    {
        _programIssue = FindProgramIssue();
        _classIssue = _ruleClass is not null && ParseClass(_ruleClass) is not null
            ? null
            : ValueIssue.Required;
        Raise(null);
        return _programIssue is null && _classIssue is null;
    }

    /// <summary>
    /// The rule the fields describe; only meaningful after <see cref="Check"/> passed.
    /// </summary>
    public HeavyRule ToRule() => new()
    {
        Program = _program,
        Args = Args.Value,
        Class = ParseClass(_ruleClass) ?? JobClass.Light,
    };

    bool? IValueHost.SwitchOf(string key) => null;

    double? IValueHost.NumberOf(string key) => null;

    IReadOnlyList<SizeRow>? IValueHost.RowsOf(string key) => null;

    string IValueHost.LabelOf(string key) => key;

    void IValueHost.Edited(ValueModel model, IReadOnlyList<Change> changes, bool now) =>
        Raise(null);

    private static JobClass? ParseClass(string? name) => name switch
    {
        "light" => JobClass.Light,
        "cpu_heavy" => JobClass.CpuHeavy,
        "gpu_heavy" => JobClass.GpuHeavy,
        "mixed" => JobClass.Mixed,
        _ => null,
    };

    private static string ClassName(JobClass jobClass) => jobClass switch
    {
        JobClass.Light => "light",
        JobClass.CpuHeavy => "cpu_heavy",
        JobClass.GpuHeavy => "gpu_heavy",
        JobClass.Mixed => "mixed",
        _ => throw new ArgumentOutOfRangeException(nameof(jobClass), jobClass, null),
    };

    private ValueIssue? FindProgramIssue()
    {
        if (_program.Length == 0)
        {
            return ValueIssue.Blank;
        }

        var pattern = ProgramSpec.Control is SettingControl.Text text ? text.Pattern : null;
        return pattern is not null
            && !Regex.IsMatch(_program, pattern, RegexOptions.CultureInvariant)
            ? ValueIssue.Pattern
            : null;
    }

    private SettingControl.Choice? Choice() => ClassSpec.Control as SettingControl.Choice;

    private SettingSpec Field(string key) =>
        _fields.FirstOrDefault(candidate => candidate.Key == key)
        ?? throw new InvalidOperationException($"a rule has no field {key}");
}
