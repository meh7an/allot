import { CancelledError } from "./results.js";
import { SERVICE_STOPPED_TEXT } from "./live-text.js";
import {
  DRAFT_NOT_READY_TEXT,
  classLabels,
  ruleArguments,
  ruleDeletedText,
  ruleMenuName,
  ruleRowName,
  testLines,
} from "./rules-text.js";
import { DurationModel, issueText } from "./setting-values.js";
import { SAVE_DELAY_MS } from "./settings-session.js";
import {
  deleteRowName,
  dialogFitsText,
  editRowName,
  resetRowName,
  rowRemovedText,
  sizeSummary,
} from "./sizes-text.js";

/**
 * @import { Change, CommandTest, HeavyRule, JobClass } from "../contract.g.mjs"
 * @import { FitPreviews } from "./fit-previews.js"
 * @import { Timers } from "./poller.js"
 * @import { NumberModel, RowDraft, RuleDraft, RuleListModel, SizeRowView, SizeTableModel,
 *   ValueModel } from "./setting-values.js"
 * @import { SettingsSession } from "./settings-session.js"
 */

// The Settings window's editors without their toolkit, shared by GNOME and KDE: a setting's
// lines under its label, the size tables' rows and edit dialog, the rule list's rows and
// actions, and Test a command.

/**
 * The lines under a setting's label: its help, then notes such as a disabled note, a restart
 * note or an issue. The key goes at the end of the help; each panel styles it.
 *
 * @param {ValueModel<unknown>} model
 * @param {(value: number) => string} number
 * @param {string | null} [note]
 * @returns {string[]} The notes after the help, without empty ones.
 */
export function settingNotes(model, number, note = null) {
  const issue = model.issue === null ? null : issueText(model.issue, number);
  return [model.disabledNote, note, issue].filter(
    /** @returns {line is string} */ (line) => line !== null && line !== "",
  );
}

/**
 * Whether a number field takes input now: a duration that is switched off does not.
 *
 * @param {NumberModel} model
 * @param {boolean} readOnly
 * @returns {boolean}
 */
export function numberEnabled(model, readOnly) {
  const on = !(model instanceof DurationModel) || model.isOn;
  return model.enabled && on && !readOnly;
}

/**
 * Names a class from the Classes table's spec, such as "CPU heavy".
 *
 * @param {SettingsSession} session
 * @returns {(jobClass: JobClass) => string}
 */
export function classLabelOf(session) {
  return classLabels(session.setting("classes")?.spec.control);
}

/**
 * Runs `work` once the user has paused for the save delay.
 *
 * @param {Timers} timers
 * @param {() => Promise<void>} work
 * @param {(error: unknown) => void} onError
 * @returns {{ run: () => void, cancel: () => void }}
 */
export function debounced(timers, work, onError) {
  /** @type {unknown} */
  let timer = null;
  const cancel = () => {
    if (timer !== null) {
      timers.clearTimeout(timer);
      timer = null;
    }
  };
  return {
    cancel,
    run: () => {
      cancel();
      timer = timers.setTimeout(() => {
        timer = null;
        work().catch(onError);
      }, SAVE_DELAY_MS);
    },
  };
}

/**
 * A request whose window may close before it answers: a cancelled one gives `null`.
 *
 * @template T
 * @param {() => Promise<T>} request
 * @returns {Promise<T | null>}
 */
export async function ask(request) {
  try {
    return await request();
  } catch (error) {
    if (error instanceof CancelledError) {
      return null;
    }
    throw error;
  }
}

/**
 * One row of a class or preset table.
 *
 * @typedef {object} SizeRowItem
 * @property {SizeRowView} view
 * @property {string} name
 * @property {string} label
 * @property {string} keyText The row's TOML name, shown for classes; empty for presets.
 * @property {string} summary Its sizes and fits in one line.
 * @property {string} editName The Edit button's accessible name.
 * @property {string | null} resetName The reset button's name, when it shows one.
 * @property {string | null} deleteName The delete button's name, when it shows one.
 */

/**
 * The rows a class or preset table shows, with their summaries and buttons.
 *
 * @param {SizeTableModel} table
 * @param {FitPreviews} fits
 * @param {(value: number) => string} number
 * @returns {SizeRowItem[]}
 */
export function sizeRowItems(table, fits, number) {
  return table.rows.map((view) => {
    const name = view.row.name;
    return {
      view,
      name,
      label: view.label,
      keyText: table.canAdd ? "" : name,
      summary: sizeSummary(view.row, fits.fit(table.key, name), number),
      editName: editRowName(table, view),
      resetName: view.builtIn && view.changed ? resetRowName(table, view) : null,
      deleteName: view.builtIn ? null : deleteRowName(view),
    };
  });
}

/**
 * Removes a class's or preset's row (a class or built-in preset goes back to its default).
 *
 * @param {SizeTableModel} table
 * @param {SizeRowView} view
 * @returns {{ text: string, undo: () => void }} The notice and its Undo.
 */
export function removeSizeRow(table, view) {
  const old = view.row;
  table.remove(view.row.name);
  return { text: rowRemovedText(table, view), undo: () => table.restore(old) };
}

/**
 * The edit dialog's live line under the sizes: how many of this row fit with the draft's
 * values, from allotd's fit preview.
 *
 * @param {SettingsSession} session
 * @param {SizeTableModel} table
 * @param {RowDraft} draft
 * @param {(value: number) => string} number
 * @returns {Promise<string | null>} `null` when there is nothing new to show: the window
 *   closed first, or allotd did not answer the preview, so the line shown stays.
 */
export async function sizeDialogFit(session, table, draft, number) {
  if (session.status !== "ready") {
    return SERVICE_STOPPED_TEXT;
  }
  const changes = table.proposal(draft);
  if (changes === null) {
    return "";
  }
  const reply = await ask(() => session.preview(changes));
  if (reply === null || !reply.ok) {
    return null;
  }
  const fits = table.key === "classes" ? reply.value.classes : reply.value.presets;
  const fit = fits.find((candidate) => candidate.name === draft.name);
  return fit === undefined ? "" : dialogFitsText(fit, number);
}

/**
 * The built-in row behind a draft, when it is changed and can go back to its default from the
 * dialog's Reset to Default.
 *
 * @param {SizeTableModel} table
 * @param {RowDraft} draft
 * @returns {SizeRowView | null}
 */
export function resettableRow(table, draft) {
  if (draft.original === null) {
    return null;
  }
  const view = table.rows.find((candidate) => candidate.row.name === draft.original);
  return view !== undefined && view.builtIn && view.changed ? view : null;
}

/**
 * One rule's row.
 *
 * @typedef {object} RuleRowItem
 * @property {HeavyRule} rule
 * @property {number} index
 * @property {string} title Its position and program, such as "1. cargo".
 * @property {string} subtitle Its arguments and class.
 * @property {string} name Its accessible name.
 * @property {string} menuName Its menu button's accessible name.
 * @property {boolean} matches Whether the command under test matched it.
 * @property {boolean} canMoveUp
 * @property {boolean} canMoveDown
 */

/**
 * @param {RuleListModel} rules
 * @param {SettingsSession} session
 * @param {number[]} matched The rules the command under test matched.
 * @returns {RuleRowItem[]}
 */
export function ruleRowItems(rules, session, matched) {
  const label = classLabelOf(session);
  const count = rules.value.length;
  return rules.value.map((rule, index) => ({
    rule,
    index,
    title: `${index + 1}. ${rule.program}`,
    subtitle: `${ruleArguments(rule)} → ${label(rule.class)}`,
    name: ruleRowName(index, rule, label),
    menuName: ruleMenuName(index),
    matches: matched.includes(index),
    canMoveUp: index > 0,
    canMoveDown: index < count - 1,
  }));
}

/**
 * Deletes a rule.
 *
 * @param {RuleListModel} rules
 * @param {number} index
 * @returns {{ focus: number, text: string, undo: () => void }} The rule to focus after the
 *   list is rebuilt, the notice and its Undo.
 */
export function deleteRule(rules, index) {
  const old = rules.value.slice();
  rules.remove(index);
  return {
    focus: Math.min(index, rules.value.length - 1),
    text: ruleDeletedText(index),
    undo: () => rules.replace(old),
  };
}

/**
 * Moves a rule up or down one place.
 *
 * @param {RuleListModel} rules
 * @param {number} from
 * @param {number} to
 * @returns {number | null} The rule to focus after the list is rebuilt; `null` past an end.
 */
export function moveRule(rules, from, to) {
  if (to < 0 || to >= rules.value.length) {
    return null;
  }
  rules.move(from, to);
  return to;
}

/**
 * What Test a command tests against: the saved rules (no changes), or a rule dialog's draft.
 *
 * @typedef {{ changes: Change[], rules: HeavyRule[] }} TestAgainst
 */

/**
 * The rules on screen, for the Commands page's box.
 *
 * @param {RuleListModel} rules
 * @returns {TestAgainst}
 */
export function savedRules(rules) {
  return { changes: [], rules: rules.value };
}

/**
 * The rules a rule dialog's draft would save, for its own box.
 *
 * @param {RuleListModel} rules
 * @param {RuleDraft} draft
 * @returns {TestAgainst | null} `null` while the draft is not ready.
 */
export function draftRules(rules, draft) {
  const changes = rules.proposal(draft);
  const set = changes?.[0];
  return set?.kind === "set_rules" ? { changes: changes ?? [], rules: set.rules } : null;
}

/**
 * Test a command: the line as the hook would treat it, in Bash, and the test
 * allotd answered, whose rules each panel marks.
 *
 * @param {SettingsSession} session
 * @param {string} line
 * @param {TestAgainst | null} against
 * @param {(value: number) => string} number
 * @returns {Promise<{ lines: string[], test: CommandTest | null } | null>} `null` when the
 *   window closed first.
 */
export async function testCommand(session, line, against, number) {
  const command = line.trim();
  if (command === "") {
    return { lines: [], test: null };
  }
  if (against === null) {
    return { lines: [DRAFT_NOT_READY_TEXT], test: null };
  }
  if (session.status !== "ready") {
    return { lines: [SERVICE_STOPPED_TEXT], test: null };
  }
  const reply = await ask(() => session.classify(command, "bash", against.changes));
  if (reply === null) {
    return null;
  }
  if (!reply.ok) {
    return { lines: [reply.failure.message], test: null };
  }
  const label = classLabelOf(session);
  return { lines: testLines(reply.value, against.rules, label, number), test: reply.value };
}
