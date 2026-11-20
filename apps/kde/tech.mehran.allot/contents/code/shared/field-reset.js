// Generated from apps/gnome/allot@mehran.tech/lib/field-reset.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import { DurationModel, NumberModel, StringListModel, SwitchModel } from "./setting-values.js";
/** @import { ValueModel } from "./setting-values.js" */
/**
 * A field's reset button: the key goes, and the notice's Undo writes back the value it had.
 *
 * @param {ValueModel<unknown>} model
 * @returns {(() => void) | null} What Undo does, or `null` when this kind of value has none.
 */
export function resetField(model) {
    const undo = undoOf(model);
    model.reset();
    return undo;
}
/**
 * Reads the saved value now, before the reset replaces it.
 *
 * @param {ValueModel<unknown>} model
 * @returns {(() => void) | null}
 */
function undoOf(model) {
    const saved = model.saved;
    if (model instanceof SwitchModel && typeof saved === "boolean") {
        return () => model.set(saved);
    }
    if (model instanceof DurationModel && model.off !== null && saved === model.off.value) {
        return () => model.setOn(false);
    }
    if (model instanceof NumberModel) {
        // The unit is read when Undo runs: the reset may have changed it.
        return typeof saved === "number"
            ? () => model.commit(saved / model.unit.factor)
            : () => model.setAutomatic(true);
    }
    if (model instanceof StringListModel && Array.isArray(saved)) {
        const entries = saved.slice();
        return () => model.replace(entries);
    }
    return null;
}
