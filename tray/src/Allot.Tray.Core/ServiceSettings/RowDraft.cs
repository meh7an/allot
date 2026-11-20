using System.Globalization;
using System.Text.RegularExpressions;

using Allot.Tray.Core.Contract;
using Allot.Tray.Core.ViewModels;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// The fields of a class or preset in its edit dialog, kept until Save. It is the
/// <see cref="IValueHost"/> of its fields, so each field checks itself against the others.
/// </summary>
public sealed class RowDraft : Observable, IValueHost
{
    private readonly SizeTableModel _table;
    private string _name;
    private ValueIssue? _nameIssue;

    internal RowDraft(SizeTableModel table, SizeRow? row, bool builtIn)
    {
        _table = table;
        Original = row?.Name;
        _name = row?.Name ?? string.Empty;
        BuiltIn = builtIn;
        var fields = new List<NumberModel>();
        foreach (var spec in table.Control.Fields)
        {
            var value = row is null ? NewRowValue(spec) : RowValue(row, spec.Key);
            fields.Add(ValueModels.NumberFor(spec, this, value));
        }

        Fields = fields;
    }

    /// <summary>
    /// The name as typed, trimmed.
    /// </summary>
    public string Name => _name;

    public ValueIssue? NameIssue => _nameIssue;

    /// <summary>
    /// The row's name before the edit, or <c>null</c> for a new row.
    /// </summary>
    public string? Original { get; }

    public bool BuiltIn { get; }

    public IReadOnlyList<NumberModel> Fields { get; }

    /// <summary>
    /// Whether the name can be edited: rows the user added, in a table that allows them.
    /// </summary>
    public bool CanRename => _table.CanAdd && !BuiltIn;

    /// <summary>
    /// The name field's label, help and pattern, when the table allows names.
    /// </summary>
    public RowName? NameSpec => _table.Control.NewRowName;

    /// <summary>
    /// Whether the row could be saved now, without showing any issue.
    /// </summary>
    public bool Ready =>
        FindNameIssue() is null
        && Fields.All(model => model.Issue is null
            && (model.Value is not null || model.Control.Automatic is not null));

    public NumberModel? Field(string key) => Fields.FirstOrDefault(field => field.Key == key);

    public void SetName(string text)
    {
        _name = text.Trim();
        _nameIssue = FindNameIssue();
        Raise(null);
    }

    /// <summary>
    /// Checks every field and the name, showing each issue.
    /// </summary>
    /// <returns>Whether the row can be saved.</returns>
    public bool Check()
    {
        _nameIssue = FindNameIssue();
        foreach (var field in Fields)
        {
            if (field.Value is null)
            {
                field.SetIssue(field.Control.Automatic is null ? ValueIssue.Required : null);
            }
            else
            {
                field.Recheck();
            }
        }

        Raise(null);
        return _nameIssue is null && Fields.All(field => field.Issue is null);
    }

    /// <summary>
    /// The row the fields describe; fields without a value read 0, or automatic for the minimum
    /// cores.
    /// </summary>
    public SizeRow ToRow()
    {
        var profile = new JobProfile
        {
            Cores = (float)FieldNumber("cores"),
            RamMb = FieldWhole("ram_mb"),
            GpuPct = (float)FieldNumber("gpu_pct"),
            VramMb = FieldWhole("vram_mb"),
            EstSecs = FieldWhole("est_secs"),
        };
        float? minCores = null;
        if (Field("min_cores")?.Value is { } minimum)
        {
            minCores = (float)minimum;
        }

        return new SizeRow { Name = _name, Profile = profile, MinCores = minCores };
    }

    bool? IValueHost.SwitchOf(string key) => null;

    double? IValueHost.NumberOf(string key) => Field(key)?.Value;

    IReadOnlyList<SizeRow>? IValueHost.RowsOf(string key) => null;

    string IValueHost.LabelOf(string key) => Field(key)?.Label ?? key;

    void IValueHost.Edited(ValueModel model, IReadOnlyList<Change> changes, bool now)
    {
        foreach (var field in Fields)
        {
            field.Recheck();
        }

        Raise(null);
    }

    private static double? NewRowValue(SettingSpec spec) =>
        spec.Control is SettingControl.Number number ? number.Default : null;

    private static double? RowValue(SizeRow row, string key) => key switch
    {
        "min_cores" => Widen(row.MinCores),
        "cores" => Widen(row.Profile.Cores),
        "ram_mb" => row.Profile.RamMb,
        "gpu_pct" => Widen(row.Profile.GpuPct),
        "vram_mb" => row.Profile.VramMb,
        "est_secs" => row.Profile.EstSecs,
        _ => null,
    };

    // A float widened to a double shows noise past its seventh digit; the shortest text that reads
    // back as the same float is what the file holds.
    private static double? Widen(float? value)
    {
        if (value is not { } number)
        {
            return null;
        }

        var text = number.ToString("R", CultureInfo.InvariantCulture);
        return double.Parse(text, CultureInfo.InvariantCulture);
    }

    private double FieldNumber(string key) => Field(key)?.Value ?? 0;

    private ulong FieldWhole(string key) =>
        (ulong)Math.Round(Math.Max(0, FieldNumber(key)), MidpointRounding.AwayFromZero);

    private ValueIssue? FindNameIssue()
    {
        if (NameSpec is not { } spec || !CanRename)
        {
            return null;
        }

        if (_name.Length == 0)
        {
            return ValueIssue.Blank;
        }

        if (!Regex.IsMatch(_name, spec.Pattern, RegexOptions.CultureInvariant))
        {
            return ValueIssue.Pattern;
        }

        return _table.TakenNames(Original).Contains(_name) ? ValueIssue.NameTaken : null;
    }
}
