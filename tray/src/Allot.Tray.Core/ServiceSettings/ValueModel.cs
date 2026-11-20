using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// One value of the Settings window, with its saved value, the value shown, and an issue when the
/// shown value cannot be saved. The models hold no I/O: their <see cref="IValueHost"/> saves what
/// they report.
/// </summary>
public abstract class ValueModel : Observable
{
    protected ValueModel(SettingSpec spec, IValueHost host)
    {
        Spec = spec;
        Host = host;
    }

    public SettingSpec Spec { get; }

    public string Key => Spec.Key;

    public string Label => Spec.Label;

    /// <summary>
    /// What is wrong with the shown value, or <c>null</c>.
    /// </summary>
    public ValueIssue? Issue { get; private set; }

    /// <summary>
    /// Bumped on every edit, so a save that answers late marks only the edit it carried as saved.
    /// </summary>
    public int Generation { get; private set; }

    /// <summary>
    /// Whether the shown value is the user's edit, not yet saved.
    /// </summary>
    public bool Dirty { get; private set; }

    /// <summary>
    /// Whether the switch this setting needs is on; a disabled setting keeps its value.
    /// </summary>
    public bool Enabled =>
        Spec.Requires is not { } requires || Host.SwitchOf(requires.Key) != false;

    /// <summary>
    /// Why the setting is disabled, or <c>null</c> while it is enabled.
    /// </summary>
    public string? DisabledNote => Enabled ? null : Spec.Requires?.Note;

    /// <summary>
    /// Whether the row shows its reset button: the saved value differs from the default.
    /// </summary>
    public abstract bool CanReset { get; }

    protected IValueHost Host { get; }

    /// <summary>
    /// Removes the key from the file, so its default applies again.
    /// </summary>
    public abstract void Reset();

    /// <summary>
    /// Shows the saved value again and drops the edit and its issue.
    /// </summary>
    public abstract void Undo();

    /// <summary>
    /// Takes the value allotd reports for this setting.
    /// </summary>
    /// <returns>False when <paramref name="value"/> is not of this model's kind.</returns>
    public abstract bool Take(SettingValue value);

    /// <summary>
    /// Marks the edit of <paramref name="generation"/> as saved; a later edit stays dirty.
    /// </summary>
    public void MarkSaved(int generation)
    {
        if (generation == Generation && Issue is null && Dirty)
        {
            Dirty = false;
            Changed();
        }
    }

    /// <summary>
    /// Shows allotd's reason for refusing the value; the value stays for the user to fix.
    /// </summary>
    public void Refuse(string message)
    {
        Issue = ValueIssue.Service(message);
        Changed();
    }

    /// <summary>
    /// allotd's reason for a refused value as a sentence that names it, such as "Headroom must be
    /// at least 0 and below 100".
    /// </summary>
    /// <param name="problemKey">The dotted key allotd reported, this setting's or inside it.</param>
    /// <param name="message">What the value must be.</param>
    public virtual string DescribeProblem(string problemKey, string message) =>
        $"{Label} {message}";

    internal void SetIssue(ValueIssue? issue)
    {
        Issue = issue;
        Changed();
    }

    /// <summary>
    /// Shows <paramref name="issue"/> without changing the value.
    /// </summary>
    protected void Flag(ValueIssue? issue) => SetIssue(issue);

    /// <summary>
    /// The shown value is now the user's edit.
    /// </summary>
    protected void MarkEdited(ValueIssue? issue)
    {
        Issue = issue;
        Dirty = true;
        Generation++;
        Changed();
    }

    /// <summary>
    /// The shown value is the saved one again.
    /// </summary>
    protected void ClearEdit()
    {
        Issue = null;
        Dirty = false;
        Generation++;
        Changed();
    }

    protected void Changed() => Raise(null);
}

/// <summary>
/// A <see cref="ValueModel"/> of one kind of value.
/// </summary>
/// <typeparam name="TValue">The value as the control edits it.</typeparam>
public abstract class ValueModel<TValue> : ValueModel
{
    private TValue _saved;
    private TValue _value;

    protected ValueModel(SettingSpec spec, IValueHost host, TValue value)
        : base(spec, host)
    {
        _saved = value;
        _value = value;
    }

    /// <summary>
    /// The value the field shows: the saved one, or the user's edit.
    /// </summary>
    public TValue Value => _value;

    /// <summary>
    /// The value allotd last reported.
    /// </summary>
    public TValue Saved => _saved;

    /// <inheritdoc/>
    public override bool CanReset => HasDefault && !Same(_saved, DefaultValue);

    /// <summary>
    /// Whether the setting has a value while its key is not in the file.
    /// </summary>
    protected abstract bool HasDefault { get; }

    /// <summary>
    /// The value while the key is not in the file; only read when <see cref="HasDefault"/>.
    /// </summary>
    protected abstract TValue DefaultValue { get; }

    /// <summary>
    /// Takes the value allotd reports. An edit not yet saved stays on screen; a hand edit to the
    /// same key then meets it as a conflict when it is saved.
    /// </summary>
    public void Load(TValue saved)
    {
        _saved = saved;
        if (!Dirty)
        {
            _value = saved;
            Loaded();
        }

        Changed();
    }

    /// <inheritdoc/>
    public override void Reset()
    {
        if (!HasDefault)
        {
            return;
        }

        Show(DefaultValue, null);
        Host.Edited(this, [new Change.Reset { Key = Key }], false);
    }

    /// <inheritdoc/>
    public override void Undo()
    {
        _value = _saved;
        Loaded();
        ClearEdit();
        Host.Edited(this, [], false);
    }

    /// <summary>
    /// Whether two values are the same, for <see cref="ValueModel.CanReset"/>.
    /// </summary>
    protected abstract bool Same(TValue left, TValue right);

    /// <summary>
    /// Called after the shown value was replaced by the saved one.
    /// </summary>
    protected virtual void Loaded()
    {
    }

    /// <summary>
    /// Shows <paramref name="value"/> as the user's edit.
    /// </summary>
    protected void Show(TValue value, ValueIssue? issue)
    {
        _value = value;
        MarkEdited(issue);
    }

    /// <summary>
    /// Shows <paramref name="value"/> and reports it to the host: saved with
    /// <paramref name="change"/> when it has no issue.
    /// </summary>
    protected void Apply(TValue value, ValueIssue? issue, Change change, bool now)
    {
        Show(value, issue);
        if (issue is null)
        {
            Host.Edited(this, [change], now);
        }
        else
        {
            Host.Edited(this, [], now);
        }
    }
}
