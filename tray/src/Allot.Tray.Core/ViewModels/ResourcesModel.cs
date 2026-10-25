using System.Collections.ObjectModel;

namespace Allot.Tray.Core.ViewModels;

/// <summary>
/// The observable Resources card. Each poll's <see cref="ResourcesView"/> is applied in place:
/// rows keep their controls while the set of rows stays the same, and only changed values raise
/// a change.
/// </summary>
public sealed class ResourcesModel : Observable
{
    private bool _visible;
    private bool _expanded;
    private string _summary = "";
    private bool _showSeparator;
    private bool _allOff;
    private bool _showLegend;

    public bool Visible
    {
        get => _visible;
        private set => Set(ref _visible, value);
    }

    public bool Expanded
    {
        get => _expanded;
        private set => Set(ref _expanded, value);
    }

    public string Summary
    {
        get => _summary;
        private set => Set(ref _summary, value);
    }

    public bool ShowSeparator
    {
        get => _showSeparator;
        private set => Set(ref _showSeparator, value);
    }

    public bool AllOff
    {
        get => _allOff;
        private set => Set(ref _allOff, value);
    }

    public bool ShowLegend
    {
        get => _showLegend;
        private set => Set(ref _showLegend, value);
    }

    public ObservableCollection<MeterRowModel> Meters { get; } = [];

    public ObservableCollection<DriveRowModel> Drives { get; } = [];

    public void Apply(ResourcesView view)
    {
        Visible = view.Visible;
        Expanded = view.Expanded;
        Summary = view.Summary;
        ShowSeparator = view.ShowSeparator;
        AllOff = view.AllOff;
        ShowLegend = view.ShowLegend;
        Sync(Meters, view.Meters, row => row.Key, row => new MeterRowModel(row),
            (model, row) => model.Update(row));
        Sync(Drives, view.Drives, row => row.Label, row => new DriveRowModel(row),
            (model, row) => model.Update(row));
    }

    // Rows change only when a toggle or the hardware does, so a different set of keys simply
    // rebuilds the list.
    private static void Sync<TModel, TRow>(
        ObservableCollection<TModel> models,
        IReadOnlyList<TRow> rows,
        Func<TRow, string> key,
        Func<TRow, TModel> create,
        Action<TModel, TRow> update)
        where TModel : IKeyed
    {
        var same = models.Select(model => model.Key)
            .SequenceEqual(rows.Select(key), StringComparer.Ordinal);
        if (!same)
        {
            models.Clear();
            foreach (var row in rows)
            {
                models.Add(create(row));
            }

            return;
        }

        for (var i = 0; i < rows.Count; i++)
        {
            update(models[i], rows[i]);
        }
    }
}

public interface IKeyed
{
    string Key { get; }
}

public sealed class MeterRowModel(MeterRow row) : Observable, IKeyed
{
    private MeterRow _row = row;

    public string Key => _row.Key;

    public string Label => _row.Label;

    public double Used => _row.Used;

    public bool HasReserved => _row.Reserved is not null;

    public double Reserved => _row.Reserved ?? 0;

    public double Headroom => _row.Headroom;

    public string Value => _row.Value;

    public string Tooltip => _row.Tooltip;

    public IReadOnlyList<double> Spark => _row.Spark;

    public bool HasSpark => _row.Spark.Count > 1;

    public void Update(MeterRow fresh)
    {
        var old = _row;
        if (old == fresh)
        {
            return;
        }

        _row = fresh;
        Changed(old.Label != fresh.Label, nameof(Label));
        Changed(old.Used != fresh.Used, nameof(Used));
        Changed((old.Reserved is null) != (fresh.Reserved is null), nameof(HasReserved));
        Changed(old.Reserved != fresh.Reserved, nameof(Reserved));
        Changed(old.Headroom != fresh.Headroom, nameof(Headroom));
        Changed(old.Value != fresh.Value, nameof(Value));
        Changed(old.Tooltip != fresh.Tooltip, nameof(Tooltip));
        Changed(!old.Spark.SequenceEqual(fresh.Spark), nameof(Spark));
        Changed((old.Spark.Count > 1) != (fresh.Spark.Count > 1), nameof(HasSpark));
    }

    private void Changed(bool changed, string name)
    {
        if (changed)
        {
            Raise(name);
        }
    }
}

public sealed class DriveRowModel(DriveRow row) : Observable, IKeyed
{
    private double _used = row.Used;
    private string _activity = row.Activity;
    private string _value = row.Value;

    public string Key { get; } = row.Label;

    public string Label => Key;

    public double Used
    {
        get => _used;
        private set => Set(ref _used, value);
    }

    public string Activity
    {
        get => _activity;
        private set => Set(ref _activity, value);
    }

    public string Value
    {
        get => _value;
        private set => Set(ref _value, value);
    }

    public void Update(DriveRow fresh)
    {
        Used = fresh.Used;
        Activity = fresh.Activity;
        Value = fresh.Value;
    }
}
