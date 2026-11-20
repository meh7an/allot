/// A field's reset button: the key goes, and the notice's Undo writes back the value it had.
@MainActor
public enum FieldReset {
    /// Resets `model` to its default.
    ///
    /// - Returns: What Undo does, or `nil` when this kind of value has no Undo.
    @discardableResult
    public static func reset(_ model: ValueModel) -> (@MainActor () -> Void)? {
        let undo = undoOf(model)
        model.reset()
        return undo
    }

    // Each case reads the saved value now, before the reset replaces it.
    private static func undoOf(_ model: ValueModel) -> (@MainActor () -> Void)? {
        switch (model, model.saved) {
        case (let switchModel as SwitchModel, .on(let on)):
            return { switchModel.set(on) }
        case (let duration as DurationModel, .number(let saved?))
        where duration.off?.value == saved:
            return { duration.setOn(false) }
        case (let number as NumberModel, .number(let saved?)):
            // The unit is read when Undo runs: the reset may have changed it.
            return { number.commit(saved / number.unit.factor) }
        case (let number as NumberModel, .number(.none)):
            return { number.setAutomatic(true) }
        case (let list as StringListModel, .list(let saved)):
            return { list.replace(saved) }
        default:
            return nil
        }
    }
}
