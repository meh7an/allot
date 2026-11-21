using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// A number in the unit of its setting: percent, runs, days, cores or a port. Durations and
/// memory sizes add a unit the user picks. <c>null</c> is a value only where the setting can be
/// automatic.
/// </summary>
public class NumberModel : ValueModel<double?>
{
    private readonly IReadOnlyList<ValueUnit> _units;

    public NumberModel(SettingSpec spec, IValueHost host, double? value)
        : this(spec, host, value, [])
    {
    }

    /// <param name="spec">The setting; its control must be a number.</param>
    /// <param name="host">Where the model lives.</param>
    /// <param name="value">The saved value.</param>
    /// <param name="units">The units the value can be shown in, smallest first.</param>
    protected NumberModel(
        SettingSpec spec,
        IValueHost host,
        double? value,
        IReadOnlyList<ValueUnit> units)
        : base(spec, host, value)
    {
        Control = spec.Control as SettingControl.Number
            ?? throw new ArgumentException($"{spec.Key} is not a number", nameof(spec));
        _units = units;
        Unit = ValueUnit.Best(units, value);
    }

    public SettingControl.Number Control { get; }

    public Quantity Quantity => Control.Quantity;

    /// <summary>
    /// The unit the value is shown in.
    /// </summary>
    public ValueUnit Unit { get; protected set; }

    /// <summary>
    /// The units the user can pick from; empty when the number has only its own.
    /// </summary>
    public IReadOnlyList<ValueUnit> Units => _units;

    /// <summary>
    /// The value in the picked unit, or <c>null</c> while automatic.
    /// </summary>
    public double? Shown => Value is { } number ? number / Unit.Factor : null;

    /// <summary>
    /// Whether the number box shows whole numbers only. A number with units shows decimals even
    /// when the file holds whole numbers, since 1.5 hours is whole seconds; <see cref="Commit"/>
    /// rounds to the file's unit.
    /// </summary>
    public bool ShowsWholeNumbers => Control.Integer && _units.Count == 0;

    /// <summary>
    /// The slider's range in whole steps: an exclusive bound is one step inside the limit, so
    /// the slider cannot reach a value the setting refuses.
    /// </summary>
    public (double Minimum, double Maximum) SliderRange
    {
        get
        {
            var low = Control.Min is { } min ? (min.Exclusive ? min.Value + 1 : min.Value) : 0;
            var high = Control.Max is { } max ? (max.Exclusive ? max.Value - 1 : max.Value) : 100;
            return (low, Math.Max(low, high));
        }
    }

    /// <summary>
    /// Whether the value is worked out by allotd, as <c>automatic</c> describes.
    /// </summary>
    public bool Automatic => Value is null;

    /// <inheritdoc/>
    protected override bool HasDefault =>
        Control.Default is not null || Control.Automatic is not null;

    /// <inheritdoc/>
    protected override double? DefaultValue => Control.Default;

    /// <summary>
    /// Shows the value in another unit; the value itself does not change.
    /// </summary>
    public void SetUnit(string name)
    {
        if (_units.FirstOrDefault(candidate => candidate.Name == name) is { } unit)
        {
            Unit = unit;
            Changed();
        }
    }

    /// <summary>
    /// Commits a number the user entered in the picked unit: on Enter, on leaving the field, or on
    /// a spin step.
    /// </summary>
    public void Commit(double shown)
    {
        // The file holds whole megabytes and seconds; a value entered in a larger unit is rounded
        // to them.
        var value = _units.Count > 0
            ? Math.Round(shown * Unit.Factor, MidpointRounding.AwayFromZero)
            : shown;
        Apply(value, Check(value), new Change.SetNumber { Key = Key, Value = value }, false);
    }

    /// <summary>
    /// Leaves the value to allotd, or gives it the number shown before.
    /// </summary>
    /// <param name="automatic">Whether allotd works the value out.</param>
    /// <param name="value">The number, in the picked unit, to give it when
    /// <paramref name="automatic"/> is false.</param>
    public void SetAutomatic(bool automatic, double? value = null)
    {
        if (automatic)
        {
            Apply(null, null, new Change.Reset { Key = Key }, false);
        }
        else if (value is { } number)
        {
            Commit(number / Unit.Factor);
        }
    }

    /// <summary>
    /// Checks the shown value again, after a value it is compared with changed.
    /// </summary>
    public void Recheck()
    {
        if (Value is { } number && Issue?.Kind != IssueKind.Service)
        {
            Flag(Check(number));
        }
    }

    /// <summary>
    /// What is wrong with <paramref name="value"/> for this setting, or <c>null</c>.
    /// </summary>
    public ValueIssue? Check(double value)
    {
        // Sizes go out as 32-bit floats, so a value that overflows one cannot be sent either.
        if (!double.IsFinite(value) || !float.IsFinite((float)value))
        {
            return ValueIssue.NotANumber;
        }

        if (Control.Integer && !ValueUnit.IsWhole(value))
        {
            return ValueIssue.Whole;
        }

        if (Outside(value, Control.Min, Control.Max))
        {
            return ValueIssue.Range(Control.Min, Control.Max);
        }

        if (Control.MinKey is { } minKey && Host.NumberOf(minKey) is { } least && value < least)
        {
            return ValueIssue.AtLeast(Host.LabelOf(minKey), least, Quantity);
        }

        if (Control.MaxKey is { } maxKey && Host.NumberOf(maxKey) is { } most && value > most)
        {
            return ValueIssue.AtMost(Host.LabelOf(maxKey), most, Quantity);
        }

        return null;
    }

    /// <inheritdoc/>
    public override bool Take(SettingValue value)
    {
        if (value is not SettingValue.NumberValue numberValue)
        {
            return false;
        }

        Load(numberValue.Value);
        return true;
    }

    /// <inheritdoc/>
    protected override bool Same(double? left, double? right) => left == right;

    /// <inheritdoc/>
    protected override void Loaded() => Unit = ValueUnit.Best(_units, Value);

    private static bool Outside(double value, Bound? min, Bound? max)
    {
        var low = min is not null && (min.Exclusive ? value <= min.Value : value < min.Value);
        var high = max is not null && (max.Exclusive ? value >= max.Value : value > max.Value);
        return low || high;
    }
}
