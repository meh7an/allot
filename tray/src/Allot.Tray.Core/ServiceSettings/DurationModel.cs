using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// A duration in seconds, shown in seconds, minutes or hours. Where one value turns the feature
/// off (<c>off</c> in the spec), a switch in front of it writes that value.
/// </summary>
public sealed class DurationModel(SettingSpec spec, IValueHost host, double? value)
    : NumberModel(spec, host, value, ValueUnit.Durations)
{
    // A duration turned back on with neither an earlier value nor a default.
    private const double MinuteSecs = 60;

    private double? _lastOn;

    /// <summary>
    /// The switch in front of the duration, or <c>null</c> when it has none.
    /// </summary>
    public OffSwitch? Off => Control.Off;

    /// <summary>
    /// Whether the feature is on; always true without an off switch.
    /// </summary>
    public bool IsOn => Off is not { } off || Value != off.Value;

    /// <summary>
    /// Turns the feature off by writing the off value, or on again with the duration it had.
    /// </summary>
    public void SetOn(bool on)
    {
        if (Off is not { } off || on == IsOn)
        {
            return;
        }

        if (!on)
        {
            _lastOn = Value;
            Apply(off.Value, null, new Change.SetNumber { Key = Key, Value = off.Value }, false);
            return;
        }

        var seconds = _lastOn ?? Control.Default ?? MinuteSecs;
        Commit(seconds / Unit.Factor);
    }

    /// <inheritdoc/>
    protected override void Loaded()
    {
        base.Loaded();
        if (Off is { } off && Value == off.Value)
        {
            Unit = ValueUnit.Best(ValueUnit.Durations, Control.Default);
        }
    }
}
