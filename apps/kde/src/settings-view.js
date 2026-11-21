import { resetField } from "../../gnome/allot@mehran.tech/lib/field-reset.js";
import { NOT_YET_TEXT } from "../../gnome/allot@mehran.tech/lib/fit-previews.js";
import {
  METERS_HELP,
  METERS_TITLE,
  NOTIFICATIONS_TITLE,
  changedPanelLines,
  meterToggles,
  notificationToggles,
  toggleEnabled,
  toggleValue,
} from "../../gnome/allot@mehran.tech/lib/panel-toggles.js";
import {
  NO_RULES_TEXT,
  matchedRules,
  ruleDialogTitle,
  rulesStatus,
  useBuiltInConfirmation,
} from "../../gnome/allot@mehran.tech/lib/rules-text.js";
import {
  DurationModel,
  NumberModel,
  StringListModel,
  SwitchModel,
  issueText,
} from "../../gnome/allot@mehran.tech/lib/setting-values.js";
import { bannerActionLabel } from "../../gnome/allot@mehran.tech/lib/settings-banners.js";
import {
  debounced,
  deleteRule,
  draftRules,
  moveRule,
  numberEnabled,
  removeSizeRow,
  resettableRow,
  ruleRowItems,
  savedRules,
  settingNotes,
  sizeDialogFit,
  sizeRowItems,
  testCommand,
} from "../../gnome/allot@mehran.tech/lib/settings-editors.js";
import {
  LOADING_TEXT,
  NOTHING_TO_RESET,
  fieldResetText,
  resetButtonName,
  resetPageLines,
} from "../../gnome/allot@mehran.tech/lib/settings-text.js";
import {
  ALLOTD_PAGES,
  TEST_COMMAND_HELP,
  TEST_COMMAND_TITLE,
  TEST_RULE_HELP,
  WHAT_FITS_TITLE,
  pageSections,
} from "../../gnome/allot@mehran.tech/lib/settings-window.js";
import {
  automaticMinCores,
  deletePresetConfirmation,
  renameWarning,
  sizeDialogTitle,
} from "../../gnome/allot@mehran.tech/lib/sizes-text.js";
import { DO_NOT_DISTURB_TEXT } from "../../gnome/allot@mehran.tech/lib/snapshot.js";

/**
 * @import { PanelToggle } from "../../gnome/allot@mehran.tech/lib/panel-toggles.js"
 * @import { SettingsBanner } from "../../gnome/allot@mehran.tech/lib/settings-banners.js"
 * @import { RowDraft, RuleDraft, RuleListModel, ValueModel }
 *   from "../../gnome/allot@mehran.tech/lib/setting-values.js"
 * @import { TestAgainst } from "../../gnome/allot@mehran.tech/lib/settings-editors.js"
 * @import { Confirmation } from "../../gnome/allot@mehran.tech/lib/settings-text.js"
 * @import { TraySettings } from "../../gnome/allot@mehran.tech/lib/settings.js"
 */

// What the KDE Settings window's QML shows, worked out here so the views only bind to it
// (DESIGN.md section 10, KDE): one plain object per row, read again whenever the window's
// revision moves on, and the shared texts and editors the views call.

export {
  ALLOTD_PAGES,
  DO_NOT_DISTURB_TEXT,
  LOADING_TEXT,
  METERS_HELP,
  METERS_TITLE,
  NOTHING_TO_RESET,
  NOTIFICATIONS_TITLE,
  NOT_YET_TEXT,
  NO_RULES_TEXT,
  TEST_COMMAND_HELP,
  TEST_COMMAND_TITLE,
  TEST_RULE_HELP,
  WHAT_FITS_TITLE,
  debounced,
  deletePresetConfirmation,
  deleteRule,
  draftRules,
  matchedRules,
  moveRule,
  pageSections,
  removeSizeRow,
  renameWarning,
  resettableRow,
  ruleDialogTitle,
  ruleRowItems,
  rulesStatus,
  savedRules,
  sizeDialogFit,
  sizeDialogTitle,
  sizeRowItems,
  testCommand,
  useBuiltInConfirmation,
};

/** The Panel page's entry in the window's page list, before allotd's pages. */
export const PANEL_PAGE_ENTRY = { id: "panel", label: "Panel", kdeIcon: "speedometer" };

/**
 * A number for the user's locale with at most two decimals, from QML's locale formatting, as
 * QML has no `Intl`.
 *
 * @param {number} value
 * @param {(value: number, decimals: number) => string} format QML's
 *   `Number(value).toLocaleString(Qt.locale(), "f", decimals)`.
 * @returns {string}
 */
export function localeNumber(value, format) {
  const rounded = Math.round(value * 100) / 100;
  const decimals = Number.isInteger(rounded) ? 0 : Number.isInteger(rounded * 10) ? 1 : 2;
  return format(rounded, decimals);
}

/**
 * What Test a command tests against: the rules on the page, or a rule dialog's draft.
 *
 * @param {RuleListModel | null} rules `null` before a rule dialog first opens.
 * @param {RuleDraft | null} draft
 * @returns {TestAgainst | null}
 */
export function testAgainst(rules, draft) {
  if (rules === null) {
    return null;
  }
  return draft === null ? savedRules(rules) : draftRules(rules, draft);
}

/**
 * Which row a setting gets.
 *
 * @param {ValueModel<unknown>} model
 * @returns {"switch" | "duration_off" | "number" | "chips" | "none"}
 */
export function rowKind(model) {
  if (model instanceof SwitchModel) {
    return "switch";
  }
  if (model instanceof DurationModel && model.off !== null) {
    return "duration_off";
  }
  if (model instanceof NumberModel) {
    return "number";
  }
  if (model instanceof StringListModel) {
    return "chips";
  }
  return "none";
}

/**
 * What every row shows: its label, its help and key, the notes under them, and its reset
 * button.
 *
 * @typedef {object} RowView
 * @property {string} label
 * @property {string} help Rich text: the help, then the key in monospace (DESIGN.md D4).
 * @property {string} key
 * @property {string} description What a screen reader says after the name: the help, the key, the
 *   notes, and the issue while one shows. Never a value that ticks (Orca reads each change).
 * @property {string[]} notes Plain lines under the help, such as a disabled or restart note.
 * @property {string | null} issue The issue line, shown with an error icon.
 * @property {boolean} enabled
 * @property {boolean} resetVisible
 * @property {string} resetName
 */

/**
 * @param {ValueModel<unknown>} model
 * @param {(value: number) => string} number
 * @param {boolean} enabled
 * @param {boolean} withReset
 * @param {boolean} readOnly
 * @param {string | null} note
 * @returns {RowView}
 */
function rowView(model, number, enabled, withReset, readOnly, note) {
  const issue = model.issue === null ? null : issueText(model.issue, number);
  const notes = settingNotes(model, number, note).filter((line) => line !== issue);
  return {
    label: model.label,
    help: helpText(model.spec.help, model.key),
    key: model.key,
    description: describe(model.spec.help, model.key, notes, issue),
    notes,
    issue,
    enabled,
    resetVisible: withReset && model.canReset && !readOnly,
    resetName: resetButtonName(model, number),
  };
}

/**
 * A control's accessible description (DESIGN.md section 11): help plus key, then the notes, then
 * the error while one shows, so a screen reader reads it when focus arrives and when it appears.
 *
 * @param {string} help
 * @param {string | null} key
 * @param {string[]} notes
 * @param {string | null} issue
 * @returns {string}
 */
export function describe(help, key, notes, issue) {
  return [help, key ?? "", ...notes, issue === null ? "" : `Error: ${issue}`]
    .filter((part) => part !== "")
    .join(" ");
}

/**
 * What a size row reads as to a screen reader: one summary, as the row shows it.
 *
 * @param {{ label: string, summary: string }} item
 * @returns {string}
 */
export function sizeRowReading(item) {
  return `${item.label}, ${item.summary}`;
}

/**
 * A help line as rich text, with the key in monospace after it.
 *
 * @param {string} help
 * @param {string | null} key
 * @returns {string}
 */
export function helpText(help, key) {
  const keyText = key === null || key === "" ? "" : `&nbsp;&nbsp;<tt>${escaped(key)}</tt>`;
  return `${escaped(help)}${keyText}`;
}

/**
 * Text that a rich-text label must not read as markup.
 *
 * @param {string} text
 * @returns {string}
 */
export function escaped(text) {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/**
 * @param {SwitchModel} model
 * @param {(value: number) => string} number
 * @param {boolean} readOnly
 * @returns {RowView & { checked: boolean }}
 */
export function switchView(model, number, readOnly) {
  const enabled = model.enabled && !readOnly;
  return { ...rowView(model, number, enabled, true, readOnly, null), checked: model.value };
}

/**
 * The switch in front of a duration that one value turns off, such as End leases left idle.
 *
 * @param {DurationModel} model
 * @param {boolean} readOnly
 * @returns {{
 *   label: string,
 *   help: string,
 *   key: string,
 *   description: string,
 *   checked: boolean,
 *   enabled: boolean,
 * }}
 */
export function offSwitchView(model, readOnly) {
  const off = model.off;
  return {
    label: off?.label ?? model.label,
    help: helpText(off?.help ?? "", model.key),
    key: model.key,
    description: describe(off?.help ?? "", model.key, [], null),
    checked: model.isOn,
    enabled: model.enabled && !readOnly,
  };
}

/**
 * A number with its unit picker.
 *
 * @typedef {RowView & {
 *   shown: number,
 *   integer: boolean,
 *   grouped: boolean,
 *   units: string[],
 *   unitIndex: number,
 *   indented: boolean,
 * }} NumberView
 */

/**
 * @param {NumberModel} model
 * @param {(value: number) => string} number
 * @param {boolean} readOnly
 * @param {string | null} note An extra line, such as Port's restart note.
 * @param {boolean} [withReset] False in an edit dialog, where a field has no default of its own.
 * @returns {NumberView}
 */
export function numberView(model, number, readOnly, note, withReset = true) {
  const units = model.units.map((unit) => unit.name);
  return {
    ...rowView(model, number, numberEnabled(model, readOnly), withReset, readOnly, note),
    shown: model.shown ?? 0,
    integer: model.control.integer,
    // DESIGN.md section 3: a port is never grouped.
    grouped: model.quantity !== "port",
    units,
    unitIndex: units.indexOf(model.unit.name),
    indented: model instanceof DurationModel && model.off !== null,
  };
}

/**
 * Chips with remove buttons, and the field that adds one.
 *
 * @param {StringListModel} model
 * @param {(value: number) => string} number
 * @param {boolean} readOnly
 * @param {boolean} [withReset]
 * @returns {RowView & { chips: Array<{ text: string, name: string }>, addLabel: string }}
 */
export function chipsView(model, number, readOnly, withReset = true) {
  return {
    ...rowView(model, number, !readOnly, withReset, readOnly, null),
    chips: model.value.map((text) => ({ text, name: `Remove ${text}` })),
    addLabel: `Add to ${model.label}`,
  };
}

/**
 * A field's reset button: the key goes, and the notice offers Undo when the value has one.
 *
 * @param {ValueModel<unknown>} model
 * @param {(value: number) => string} number
 * @returns {{ text: string, undo: (() => void) | null }}
 */
export function resetSetting(model, number) {
  const text = fieldResetText(model, number);
  return { text, undo: resetField(model) };
}

/**
 * The size dialog's Automatic minimum switch and its line.
 *
 * @param {RowDraft} draft
 * @param {NumberModel} field
 * @param {(value: number) => string} number
 * @returns {{ automatic: boolean, help: string, automaticValue: number }}
 */
export function minimumView(draft, field, number) {
  const automaticValue = automaticMinCores(draft.field("cores")?.value ?? 0);
  return {
    automatic: field.automatic,
    help: `${number(automaticValue)} cores: ${field.control.automatic ?? ""}`,
    automaticValue,
  };
}

/**
 * The size dialog's name field, for a preset that can be renamed.
 *
 * @param {RowDraft} draft
 * @param {(value: number) => string} number
 * @returns {{ label: string, help: string, description: string, issue: string | null } | null}
 */
export function nameView(draft, number) {
  const spec = draft.nameSpec;
  if (spec === null || !draft.canRename) {
    return null;
  }
  const issue = draft.nameIssue === null ? null : issueText(draft.nameIssue, number);
  return {
    label: spec.label,
    help: spec.help,
    description: describe(spec.help, "", [], issue),
    issue,
  };
}

/**
 * The rule dialog's program field.
 *
 * @param {RuleDraft} draft
 * @param {(value: number) => string} number
 * @returns {{ label: string, help: string, description: string, issue: string | null }}
 */
export function programView(draft, number) {
  const issue = draft.programIssue === null ? null : issueText(draft.programIssue, number);
  const help = draft.programSpec.help;
  return {
    label: draft.programSpec.label,
    help,
    description: describe(help, "", [], issue),
    issue,
  };
}

/**
 * @typedef {object} BannerView
 * @property {string} severity `info`, `warning` or `error`.
 * @property {string} text
 * @property {Array<{ id: string, label: string }>} actions
 */

/**
 * The banners at the top of a page, with their buttons' titles.
 *
 * @param {SettingsBanner[]} banners
 * @returns {BannerView[]}
 */
export function bannerViews(banners) {
  return banners.map((banner) => ({
    severity: banner.severity,
    text: banner.message,
    actions: banner.actions.map((id) => ({ id, label: bannerActionLabel(id) })),
  }));
}

/**
 * One switch on the Panel page.
 *
 * @typedef {object} PanelRow
 * @property {PanelToggle} toggle
 * @property {string} label
 * @property {string} help
 * @property {boolean} checked
 * @property {boolean} enabled
 * @property {boolean} indented
 */

/**
 * The Panel page's Meters and Notifications switches.
 *
 * @param {TraySettings} settings
 * @param {boolean} sharesMemory
 * @returns {{ meters: PanelRow[], notifications: PanelRow[], canReset: boolean }}
 */
export function panelView(settings, sharesMemory) {
  /** @param {PanelToggle} toggle @returns {PanelRow} */
  const row = (toggle) => ({
    toggle,
    label: toggle.label,
    help: toggle.help,
    checked: toggleValue(settings, toggle),
    enabled: toggleEnabled(settings, toggle),
    indented: toggle.parent !== null,
  });
  return {
    meters: meterToggles(sharesMemory).map(row),
    notifications: notificationToggles().map(row),
    canReset: changedPanelLines(settings, sharesMemory).length > 0,
  };
}

/**
 * The Panel page's Reset Page… confirmation, or `null` with nothing to reset.
 *
 * @param {TraySettings} settings
 * @param {boolean} sharesMemory
 * @returns {Confirmation | null}
 */
export function panelResetConfirmation(settings, sharesMemory) {
  const lines = changedPanelLines(settings, sharesMemory);
  return lines.length === 0 ? null : resetPageLines("Panel", lines);
}

/**
 * A changing value the views key a list on, so a list rebuilds only when its rows change and
 * focus survives the 5 s refresh.
 *
 * @param {unknown} value
 * @returns {string}
 */
export function signature(value) {
  return JSON.stringify(value);
}
