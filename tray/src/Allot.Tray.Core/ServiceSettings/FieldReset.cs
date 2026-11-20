namespace Allot.Tray.Core.ServiceSettings;

/// <summary>
/// A field's reset button: the key goes, and the toast's Undo writes back the value it had.
/// </summary>
public static class FieldReset
{
    /// <summary>
    /// Resets <paramref name="model"/> to its default.
    /// </summary>
    /// <returns>What Undo does, or <c>null</c> when this kind of value has no Undo.</returns>
    public static Action? Reset(ValueModel model)
    {
        var undo = UndoOf(model);
        model.Reset();
        return undo;
    }

    // Each case reads the saved value now, before the reset replaces it.
    private static Action? UndoOf(ValueModel model) => model switch
    {
        SwitchModel switchModel => TurnBack(switchModel, switchModel.Saved),
        DurationModel { Off: { } off } duration when duration.Saved == off.Value =>
            () => duration.SetOn(false),

        // The unit is read when Undo runs: the reset may have changed it.
        NumberModel { Saved: { } saved } number => () => number.Commit(saved / number.Unit.Factor),
        NumberModel number => () => number.SetAutomatic(true),
        StringListModel list => Replace(list, list.Saved),
        _ => null,
    };

    private static Action TurnBack(SwitchModel model, bool saved) => () => model.Turn(saved);

    private static Action Replace(StringListModel model, IReadOnlyList<string> saved) =>
        () => model.Replace(saved);
}
