using Allot.Tray.Core.Contract;

namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// Makes the view model of a setting, by its control and quantity.
/// </summary>
public static class ValueModels
{
    /// <summary>
    /// The view model of <paramref name="spec"/> holding <paramref name="value"/>.
    /// </summary>
    /// <returns><c>null</c> when the value is not of the kind the control edits, or the control
    /// is a text or a choice, which live only in a rule's dialog.</returns>
    public static ValueModel? Create(SettingSpec spec, IValueHost host, SettingValue value) =>
        (spec.Control, value) switch
        {
            (SettingControl.Switch, SettingValue.SwitchValue switchValue) =>
                new SwitchModel(spec, host, switchValue.Value),
            (SettingControl.Number, SettingValue.NumberValue numberValue) =>
                NumberFor(spec, host, numberValue.Value),
            (SettingControl.StringList, SettingValue.ListValue listValue) =>
                new StringListModel(spec, host, listValue.Value),
            (SettingControl.SizeTable, SettingValue.RowsValue rowsValue) =>
                new SizeTableModel(spec, host, rowsValue.Value),
            (SettingControl.RuleList, SettingValue.RulesValue rulesValue) =>
                new RuleListModel(spec, host, rulesValue.Value),
            _ => null,
        };

    /// <summary>
    /// The model of a number: a duration or a memory size by its quantity, else a plain number.
    /// </summary>
    /// <exception cref="ArgumentException">The control of <paramref name="spec"/> is not a
    /// number.</exception>
    public static NumberModel NumberFor(SettingSpec spec, IValueHost host, double? value)
    {
        Quantity? quantity = null;
        if (spec.Control is SettingControl.Number number)
        {
            quantity = number.Quantity;
        }

        return quantity switch
        {
            Quantity.Seconds => new DurationModel(spec, host, value),
            Quantity.Megabytes => new MemoryModel(spec, host, value),
            _ => new NumberModel(spec, host, value),
        };
    }

    /// <summary>
    /// The key of a value, or <c>null</c> for a kind of value this panel does not know.
    /// </summary>
    public static string? KeyOf(SettingValue value) => value switch
    {
        SettingValue.SwitchValue switchValue => switchValue.Key,
        SettingValue.NumberValue numberValue => numberValue.Key,
        SettingValue.ListValue listValue => listValue.Key,
        SettingValue.RulesValue rulesValue => rulesValue.Key,
        SettingValue.RowsValue rowsValue => rowsValue.Key,
        _ => null,
    };
}
