using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// An on/off setting.
/// </summary>
public sealed class SwitchModel(SettingSpec spec, IValueHost host, bool value)
    : ValueModel<bool>(spec, host, value)
{
    /// <inheritdoc/>
    protected override bool HasDefault => Spec.Control is SettingControl.Switch;

    /// <inheritdoc/>
    protected override bool DefaultValue => Spec.Control is SettingControl.Switch { Default: true };

    /// <summary>
    /// Turns the switch on or off.
    /// </summary>
    public void Turn(bool on) =>
        Apply(on, null, new Change.SetSwitch { Key = Key, Value = on }, false);

    /// <inheritdoc/>
    public override bool Take(SettingValue value)
    {
        if (value is not SettingValue.SwitchValue switchValue)
        {
            return false;
        }

        Load(switchValue.Value);
        return true;
    }

    /// <inheritdoc/>
    protected override bool Same(bool left, bool right) => left == right;
}
