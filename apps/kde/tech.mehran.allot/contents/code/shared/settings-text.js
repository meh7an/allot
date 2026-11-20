// Generated from apps/gnome/allot@mehran.tech/lib/settings-text.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import { DurationModel, NumberModel, quantityText } from "./setting-values.js";
/**
 * @import { Change } from "../contract.g.mjs"
 * @import { ValueModel } from "./setting-values.js"
 * @import { SaveStatus } from "./settings-session.js"
 */
// The Settings window's fixed sentences: the save status, the reset confirmations and notices,
// and a saved or pending value as the user reads it. Buttons are title case, as GNOME's and
// KDE's are (DESIGN.md section 12, as Mehran chose for M6.1).
/**
 * A confirmation, such as Reset Page or Stop Service.
 *
 * @typedef {object} Confirmation
 * @property {string} title
 * @property {string} body What happens, such as what goes back to its default.
 * @property {string} button The primary button, in title case.
 */
export const NOTHING_TO_RESET = "Nothing to reset";
export const LOADING_TEXT = "Loading settings…";
/**
 * A number for the user's locale, grouped, with at most two decimals. GJS has `Intl`; QML does
 * not, so the KDE widget passes its own formatter instead.
 *
 * @param {string | undefined} [locale] A BCP 47 tag; the system's locale when left out.
 * @returns {(value: number) => string}
 */
export function localeNumbers(locale) {
    const format = new Intl.NumberFormat(locale, { maximumFractionDigits: 2 });
    return (value) => format.format(value);
}
/**
 * "All changes saved", "Saving…", "1 change not saved".
 *
 * @param {SaveStatus} status
 * @returns {string}
 */
export function saveStatusText(status) {
    switch (status.kind) {
        case "saved":
            return "All changes saved";
        case "saving":
            return "Saving…";
        case "unsaved":
            return status.count === 1 ? "1 change not saved" : `${status.count} changes not saved`;
    }
}
/**
 * The default of a setting with its unit, such as "15 %" or "Off"; empty without one.
 *
 * @param {ValueModel<unknown>} model
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function defaultText(model, number) {
    const control = model.spec.control;
    switch (control.kind) {
        case "switch":
            return onOff(control.default);
        case "number": {
            const value = control.default;
            if (value === null) {
                return "";
            }
            if (control.off !== null && value === control.off.value) {
                return "Off";
            }
            return quantityText(value, control.quantity, number);
        }
        case "string_list":
            return control.default.join(", ");
        default:
            return "";
    }
}
/**
 * The value a change would write, for the conflict banner: "15 %", "On", "the default".
 *
 * @param {ValueModel<unknown>} model
 * @param {Change} change
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function changeValueText(model, change, number) {
    var _a;
    switch (change.kind) {
        case "set_switch":
            return onOff(change.value);
        case "set_number":
            if (model instanceof DurationModel && change.value === ((_a = model.off) === null || _a === void 0 ? void 0 : _a.value)) {
                return "Off";
            }
            return model instanceof NumberModel
                ? quantityText(change.value, model.quantity, number)
                : number(change.value);
        case "set_list":
            return change.value.join(", ");
        default:
            return "the default";
    }
}
/**
 * The notice after a field's reset: "Headroom reset to 15 %."
 *
 * @param {ValueModel<unknown>} model
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function fieldResetText(model, number) {
    const value = defaultText(model, number);
    return value === "" ? `${model.label} reset.` : `${model.label} reset to ${value}.`;
}
/**
 * The accessible name of a field's reset button: "Reset Headroom to 15 %".
 *
 * @param {ValueModel<unknown>} model
 * @param {(value: number) => string} number
 * @returns {string}
 */
export function resetButtonName(model, number) {
    const value = defaultText(model, number);
    return value === "" ? `Reset ${model.label}` : `Reset ${model.label} to ${value}`;
}
/**
 * Reset Page's confirmation for the settings on a page that differ from their default.
 *
 * @param {string} pageLabel The page, such as Admission.
 * @param {ValueModel<unknown>[]} changed
 * @param {(value: number) => string} number
 * @returns {Confirmation}
 */
export function resetPageConfirmation(pageLabel, changed, number) {
    return resetPageLines(pageLabel, changed.map((model) => {
        const value = defaultText(model, number);
        return value === "" ? model.label : `${model.label}: ${value}`;
    }));
}
/**
 * Reset Page's confirmation, from one line per changed setting with its default.
 *
 * @param {string} pageLabel
 * @param {string[]} lines
 * @returns {Confirmation}
 */
export function resetPageLines(pageLabel, lines) {
    const count = settingsCount(lines.length);
    return {
        title: `Reset ${count} on ${pageLabel}?`,
        body: `These go back to their defaults:\n${lines.join("\n")}`,
        button: `Reset ${titled(count)}`,
    };
}
/**
 * Reset All Service Settings' confirmation.
 *
 * @param {number} changed
 * @returns {Confirmation}
 */
export function resetAllConfirmation(changed) {
    return {
        title: changed === 1 ? "Reset the 1 changed setting?" : `Reset all ${changed} changed settings?`,
        body: "config.toml goes back to the defaults. Your rules and presets are replaced by the " +
            "built-in ones, and presets you added are deleted. Comments next to these keys are " +
            "removed too. This can't be undone.",
        button: `Reset ${titled(settingsCount(changed))}`,
    };
}
/**
 * @param {boolean} on
 * @returns {string}
 */
export function onOff(on) {
    return on ? "On" : "Off";
}
/**
 * "a", "a and b", "a, b and c".
 *
 * @param {string[]} parts
 * @returns {string}
 */
export function joined(parts) {
    if (parts.length < 2) {
        return parts.join("");
    }
    return `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
}
/**
 * @param {number} count
 * @returns {string}
 */
function settingsCount(count) {
    return count === 1 ? "1 setting" : `${count} settings`;
}
/**
 * "Reset 3 Settings": buttons are title case.
 *
 * @param {string} text
 * @returns {string}
 */
function titled(text) {
    return text.replace("setting", "Setting");
}
