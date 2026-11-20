using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// One row of a size table as a page lists it.
/// </summary>
/// <param name="Row">The row.</param>
/// <param name="Label">The row's label: its name for a row the user added.</param>
/// <param name="BuiltIn">Built-in rows can be edited and reset, never renamed or deleted.</param>
/// <param name="Changed">A built-in row whose values differ from its defaults; always true for
/// a row the user added.</param>
public sealed record SizeRowView(SizeRow Row, string Label, bool BuiltIn, bool Changed);

/// <summary>
/// A size table, classes or presets: rows edited one at a time in a dialog, each Save written at
/// once.
/// </summary>
public sealed class SizeTableModel : ValueModel<IReadOnlyList<SizeRow>>
{
    public SizeTableModel(SettingSpec spec, IValueHost host, IReadOnlyList<SizeRow> value)
        : base(spec, host, value)
    {
        Control = spec.Control as SettingControl.SizeTable
            ?? throw new ArgumentException($"{spec.Key} is not a size table", nameof(spec));
    }

    public SettingControl.SizeTable Control { get; }

    /// <inheritdoc/>
    public override bool CanReset => Rows.Any(row => row.Changed);

    /// <summary>
    /// Whether the user can add, rename and delete rows.
    /// </summary>
    public bool CanAdd => Control.NewRowName is not null;

    /// <summary>
    /// The rows as the page lists them.
    /// </summary>
    public IReadOnlyList<SizeRowView> Rows => [.. Value.Select(ToView)];

    /// <inheritdoc/>
    protected override bool HasDefault => true;

    /// <inheritdoc/>
    protected override IReadOnlyList<SizeRow> DefaultValue =>
        [.. Control.Rows.Select(BuiltInRow)];

    /// <summary>
    /// A draft of the row <paramref name="name"/> for its edit dialog.
    /// </summary>
    /// <returns><c>null</c> when the table has no such row.</returns>
    public RowDraft? Edit(string name)
    {
        var row = Value.FirstOrDefault(candidate => candidate.Name == name);
        return row is null ? null : new RowDraft(this, row, BuiltIn(name) is not null);
    }

    /// <summary>
    /// A draft of a new row, with the defaults a new row starts with.
    /// </summary>
    public RowDraft Add() => new(this, null, false);

    /// <summary>
    /// Writes the draft's row, renaming it when its name changed. Nothing is written while the
    /// draft has an issue.
    /// </summary>
    /// <returns>Whether it was handed over to be written.</returns>
    public bool Save(RowDraft draft)
    {
        if (!draft.Check())
        {
            return false;
        }

        var row = draft.ToRow();
        var others = Value.Where(
            candidate => candidate.Name != draft.Original && candidate.Name != row.Name);
        Write(SortRows([.. others, row]), ChangesFor(draft.Original, row));
        return true;
    }

    /// <summary>
    /// What the table would be with the draft saved, for the dialog's fit preview.
    /// </summary>
    /// <returns><c>null</c> while the draft could not be saved.</returns>
    public IReadOnlyList<Change>? Proposal(RowDraft draft) =>
        draft.Ready ? ChangesFor(draft.Original, draft.ToRow()) : null;

    /// <summary>
    /// Writes <paramref name="row"/> back as it was, such as before a reset or delete for Undo.
    /// </summary>
    public void Restore(SizeRow row)
    {
        var others = Value.Where(candidate => candidate.Name != row.Name);
        Write(SortRows([.. others, row]), ChangesFor(null, row));
    }

    /// <summary>
    /// Deletes a row the user added; a built-in row is reset instead.
    /// </summary>
    public void Remove(string name)
    {
        var rows = new List<SizeRow>();
        var spec = BuiltIn(name);
        foreach (var row in Value)
        {
            if (row.Name != name)
            {
                rows.Add(row);
            }
            else if (spec is not null)
            {
                rows.Add(BuiltInRow(spec));
            }
        }

        Write(rows, [new Change.ResetRow { Table = Key, Name = name }]);
    }

    /// <summary>
    /// Names the row and field of <c>presets.&lt;name&gt;.&lt;field&gt;</c>, or the name itself
    /// for <c>presets.&lt;name&gt;</c>.
    /// </summary>
    public override string DescribeProblem(string problemKey, string message)
    {
        var inside = problemKey[Math.Min(Key.Length + 1, problemKey.Length)..];
        if (inside.Length == 0)
        {
            return base.DescribeProblem(problemKey, message);
        }

        var dot = inside.IndexOf('.', StringComparison.Ordinal);
        var name = dot < 0 ? inside : inside[..dot];
        var fieldKey = dot < 0 ? null : inside[(dot + 1)..];
        var field = Control.Fields.FirstOrDefault(candidate => candidate.Key == fieldKey);
        var label = field?.Label ?? Control.NewRowName?.Label ?? "Name";
        return $"{BuiltIn(name)?.Label ?? name}: {label} {message}";
    }

    /// <summary>
    /// The names a new or renamed row cannot take: every other row's, and every class's.
    /// </summary>
    /// <param name="original">The row's own name, which it may keep.</param>
    public IReadOnlyList<string> TakenNames(string? original)
    {
        var names = Value
            .Select(row => row.Name)
            .Where(name => name != original)
            .ToList();
        if (Key != "classes" && Host.RowsOf("classes") is { } classes)
        {
            names.AddRange(classes.Select(row => row.Name));
        }

        return names;
    }

    /// <inheritdoc/>
    public override bool Take(SettingValue value)
    {
        if (value is not SettingValue.RowsValue rowsValue)
        {
            return false;
        }

        Load(rowsValue.Value);
        return true;
    }

    /// <inheritdoc/>
    protected override bool Same(IReadOnlyList<SizeRow> left, IReadOnlyList<SizeRow> right) =>
        left.SequenceEqual(right);

    private static SizeRow BuiltInRow(SizeRowSpec spec) =>
        new() { Name = spec.Name, Profile = spec.Default, MinCores = spec.DefaultMinCores };

    private SizeRowView ToView(SizeRow row)
    {
        var spec = BuiltIn(row.Name);
        return new SizeRowView(
            row,
            spec?.Label ?? row.Name,
            spec is not null,
            spec is null || row != BuiltInRow(spec));
    }

    // A rename removes the old row in the same write.
    private List<Change> ChangesFor(string? original, SizeRow row)
    {
        var changes = new List<Change>();
        if (original is not null && original != row.Name)
        {
            changes.Add(new Change.ResetRow { Table = Key, Name = original });
        }

        changes.Add(new Change.SetRow
        {
            Table = Key,
            Name = row.Name,
            Profile = row.Profile,
            MinCores = row.MinCores,
        });
        return changes;
    }

    private void Write(IReadOnlyList<SizeRow> rows, IReadOnlyList<Change> changes)
    {
        Show(rows, null);
        Host.Edited(this, changes, true);
    }

    // Classes keep the spec's order; presets go by name, as allotd lists them.
    private IReadOnlyList<SizeRow> SortRows(IEnumerable<SizeRow> rows)
    {
        if (Key == "classes")
        {
            var order = Control.Rows.Select(row => row.Name).ToList();
            return [.. rows.OrderBy(row => order.IndexOf(row.Name))];
        }

        return [.. rows.OrderBy(row => row.Name, StringComparer.Ordinal)];
    }

    private SizeRowSpec? BuiltIn(string name) =>
        Control.Rows.FirstOrDefault(row => row.Name == name);
}
