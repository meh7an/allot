using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// A list of strings shown as chips, such as the print-only flags; saved on add or remove.
/// </summary>
public sealed class StringListModel(SettingSpec spec, IValueHost host, IReadOnlyList<string> value)
    : ValueModel<IReadOnlyList<string>>(spec, host, value)
{
    /// <inheritdoc/>
    protected override bool HasDefault => Spec.Control is SettingControl.StringList;

    /// <inheritdoc/>
    protected override IReadOnlyList<string> DefaultValue =>
        Spec.Control is SettingControl.StringList list ? list.Default : [];

    /// <summary>
    /// Adds an entry; refuses a blank one or one the list already has.
    /// </summary>
    /// <returns>Whether it was added.</returns>
    public bool Add(string text)
    {
        var entry = text.Trim();
        if (entry.Length == 0)
        {
            Flag(ValueIssue.Blank);
            return false;
        }

        if (Value.Contains(entry))
        {
            Flag(ValueIssue.Duplicate);
            return false;
        }

        Save([.. Value, entry]);
        return true;
    }

    /// <summary>
    /// The entry text changed: a blank or duplicate flag was about the text before, so it goes.
    /// </summary>
    public void EntryChanged()
    {
        if (Issue?.Kind is IssueKind.Blank or IssueKind.Duplicate)
        {
            Flag(null);
        }
    }

    /// <summary>
    /// Writes <paramref name="entries"/> as they are, such as the list before a reset for Undo.
    /// </summary>
    public void Replace(IReadOnlyList<string> entries) => Save(entries);

    public void Remove(int index)
    {
        if (index >= 0 && index < Value.Count)
        {
            Save([.. Value.Where((_, at) => at != index)]);
        }
    }

    /// <inheritdoc/>
    public override bool Take(SettingValue value)
    {
        if (value is not SettingValue.ListValue listValue)
        {
            return false;
        }

        Load(listValue.Value);
        return true;
    }

    /// <inheritdoc/>
    protected override bool Same(IReadOnlyList<string> left, IReadOnlyList<string> right) =>
        left.SequenceEqual(right);

    private void Save(IReadOnlyList<string> entries) =>
        Apply(entries, null, new Change.SetList { Key = Key, Value = entries }, true);
}
