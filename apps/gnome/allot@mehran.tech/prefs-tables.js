import Adw from "gi://Adw";
import Gdk from "gi://Gdk";
import Gio from "gi://Gio";
import Gtk from "gi://Gtk";

import { CancelledError } from "./lib/client.js";
import { NOT_YET_TEXT } from "./lib/fit-previews.js";
import { SERVICE_STOPPED_TEXT } from "./lib/live-text.js";
import {
  DRAFT_NOT_READY_TEXT,
  NO_RULES_TEXT,
  classLabels,
  ruleArguments,
  ruleDeletedText,
  ruleDialogTitle,
  ruleMenuName,
  ruleRowName,
  rulesStatus,
  testLines,
  useBuiltInConfirmation,
} from "./lib/rules-text.js";
import { issueText } from "./lib/setting-values.js";
import { SAVE_DELAY_MS } from "./lib/settings-session.js";
import {
  automaticMinCores,
  deletePresetConfirmation,
  deleteRowName,
  dialogFitsText,
  editRowName,
  renameWarning,
  resetRowName,
  rowRemovedText,
  sizeDialogTitle,
  sizeSummary,
} from "./lib/sizes-text.js";
import {
  chipsRows,
  confirm,
  dimLabel,
  escaped,
  helpMarkup,
  iconButton,
  numberRow,
} from "./prefs-widgets.js";

/**
 * @import { Change, CommandTest, HeavyRule, JobClass } from "./contract.g.mjs"
 * @import { FitPreviews } from "./lib/fit-previews.js"
 * @import { Timers } from "./lib/poller.js"
 * @import { NumberModel, RowDraft, RuleDraft, RuleListModel, SizeRowView, SizeTableModel }
 *   from "./lib/setting-values.js"
 * @import { SettingsSession } from "./lib/settings-session.js"
 * @import { RowContext, Sync } from "./prefs-widgets.js"
 */

// The Sizes and Commands pages' sections on GNOME (DESIGN.md sections 4 and 10): a class or
// preset is a row with a summary and an edit dialog, a rule is a row with a menu and its own
// dialog, and Test a command sits above the rules. A table or list is rebuilt only when its rows
// change, so focus survives the 5 s refresh.

/**
 * @typedef {RowContext & {
 *   session: SettingsSession,
 *   fits: FitPreviews,
 *   timers: Timers,
 *   parent: () => Gtk.Widget,
 * }} TableContext
 */

const DIALOG_WIDTH = 520;

/**
 * The rows of a class or preset table, with Add Preset… below a table that allows it.
 *
 * @param {TableContext} context
 * @param {Adw.PreferencesGroup} group
 * @param {SizeTableModel} table
 * @returns {Sync}
 */
export function sizeTableRows(context, group, table) {
  /** @type {Gtk.Widget[]} */
  let shown = [];
  let signature = "";
  return () => {
    const readOnly = context.readOnly();
    const views = table.rows;
    const summaries = views.map((view) =>
      sizeSummary(view.row, context.fits.fit(table.key, view.row.name), context.number)
    );
    const next = JSON.stringify([table.value, summaries, readOnly]);
    if (next === signature) {
      return;
    }
    signature = next;
    for (const widget of shown) {
      group.remove(widget);
    }
    shown = views.map((view, index) =>
      sizeRow(context, table, view, summaries[index] ?? "", readOnly)
    );
    if (table.canAdd) {
      const add = new Adw.ButtonRow({
        title: "Add Preset…",
        start_icon_name: "list-add-symbolic",
        sensitive: !readOnly,
      });
      add.connect("activated", () => openSizeDialog(context, table, table.add()));
      shown.push(add);
    }
    for (const widget of shown) {
      group.add(widget);
    }
  };
}

/**
 * @param {TableContext} context
 * @param {SizeTableModel} table
 * @param {SizeRowView} view
 * @param {string} summary
 * @param {boolean} readOnly
 * @returns {Adw.ActionRow}
 */
function sizeRow(context, table, view, summary, readOnly) {
  const name = view.row.name;
  const key = table.canAdd ? "" : `<small><tt>${escaped(name)}</tt></small>  `;
  const row = new Adw.ActionRow({
    title: escaped(view.label),
    subtitle: `${key}${escaped(summary)}`,
    subtitle_lines: 0,
  });
  const old = view.row;
  const removed = () => {
    context.toaster.show(rowRemovedText(table, view), () => table.restore(old));
  };
  if (view.builtIn && view.changed) {
    row.add_suffix(
      iconButton("edit-undo-symbolic", resetRowName(table, view), () => {
        table.remove(name);
        removed();
      }),
    );
  }
  if (!view.builtIn) {
    row.add_suffix(
      iconButton("user-trash-symbolic", deleteRowName(view), () =>
        confirm(context.parent(), deletePresetConfirmation(name), () => {
          table.remove(name);
          removed();
        })),
    );
  }
  const edit = iconButton("document-edit-symbolic", editRowName(table, view), () => {
    const draft = table.edit(name);
    if (draft !== null) {
      openSizeDialog(context, table, draft);
    }
  });
  row.add_suffix(edit);
  row.activatable_widget = edit;
  row.sensitive = !readOnly;
  return row;
}

/**
 * The edit dialog of a class or preset: Save writes the whole row at once (DESIGN.md D2, D5).
 *
 * @param {TableContext} context
 * @param {SizeTableModel} table
 * @param {RowDraft} draft
 */
function openSizeDialog(context, table, draft) {
  const shell = dialogShell(sizeDialogTitle(table, draft));
  const { dialog, page, save } = shell;
  /** @type {Sync[]} */
  const syncs = [];
  const sync = () => {
    for (const part of syncs) {
      part();
    }
  };
  /** @type {RowContext} */
  const rows = {
    number: context.number,
    readOnly: () => false,
    toaster: context.toaster,
    changed: sync,
    noResets: true,
  };

  /** @type {Gtk.Widget | null} */
  let first = null;
  const nameSpec = draft.nameSpec;
  // Rows go in a group's list and other widgets after it, so the name's help line needs a group
  // of its own to sit right under the name.
  if (nameSpec !== null && draft.canRename) {
    const names = new Adw.PreferencesGroup();
    const entry = new Adw.EntryRow({ title: escaped(nameSpec.label), text: draft.name });
    entry.connect("changed", () => draft.setName(entry.text));
    const help = dimLabel();
    help.margin_top = 6;
    names.add(entry);
    names.add(help);
    page.add(names);
    first = entry;
    syncs.push(() => {
      const issue = draft.nameIssue === null ? null : issueText(draft.nameIssue, context.number);
      help.label = issue === null ? nameSpec.help : `${nameSpec.help}\n${issue}`;
    });
  }

  const sizes = new Adw.PreferencesGroup();
  page.add(sizes);
  for (const field of draft.fields) {
    if (field.key === "min_cores") {
      syncs.push(minimumRows(rows, sizes, draft, field));
      continue;
    }
    const number = numberRow(rows, field);
    sizes.add(number.row);
    syncs.push(number.sync);
    first ??= number.row;
  }

  const live = dimLabel();
  const liveGroup = new Adw.PreferencesGroup();
  liveGroup.add(live);
  page.add(liveGroup);
  let fitText = NOT_YET_TEXT;
  syncs.push(() => {
    live.label = renameWarning(draft) ?? fitText;
  });
  const preview = debounced(context.timers, async () => {
    if (context.session.status !== "ready") {
      fitText = SERVICE_STOPPED_TEXT;
      sync();
      return;
    }
    const changes = table.proposal(draft);
    if (changes === null) {
      fitText = "";
      sync();
      return;
    }
    const reply = await ask(() => context.session.preview(changes));
    if (reply?.ok) {
      const fits = table.key === "classes" ? reply.value.classes : reply.value.presets;
      const fit = fits.find((candidate) => candidate.name === draft.name);
      fitText = fit === undefined ? "" : dialogFitsText(fit, context.number);
    }
    sync();
  });

  if (draft.original !== null) {
    const view = table.rows.find((candidate) => candidate.row.name === draft.original);
    if (view !== undefined && view.builtIn && view.changed) {
      const footer = new Adw.PreferencesGroup();
      const reset = new Adw.ButtonRow({ title: "Reset to Default" });
      reset.connect("activated", () => {
        const old = view.row;
        table.remove(view.row.name);
        context.toaster.show(rowRemovedText(table, view), () => table.restore(old));
        dialog.close();
      });
      footer.add(reset);
      page.add(footer);
    }
  }

  draft.onChange = () => {
    sync();
    preview.run();
  };
  save.connect("clicked", () => {
    if (table.save(draft)) {
      dialog.close();
    } else {
      sync();
    }
  });
  dialog.connect("closed", () => {
    draft.onChange = null;
    preview.cancel();
  });
  dialog.focus_widget = first;
  sync();
  preview.run();
  dialog.present(context.parent());
}

/**
 * Minimum cores: Automatic, worked out from the cores, or a number of the user's.
 *
 * @param {RowContext} rows
 * @param {Adw.PreferencesGroup} group
 * @param {RowDraft} draft
 * @param {NumberModel} field
 * @returns {Sync}
 */
function minimumRows(rows, group, draft, field) {
  const automatic = new Adw.SwitchRow({ title: "Automatic minimum" });
  const number = numberRow(rows, field);
  const cores = () => draft.field("cores")?.value ?? 0;
  let syncing = false;
  automatic.connect("notify::active", () => {
    if (!syncing) {
      field.setAutomatic(automatic.active, automaticMinCores(cores()));
    }
  });
  group.add(automatic);
  group.add(number.row);
  return () => {
    syncing = true;
    automatic.active = field.automatic;
    syncing = false;
    const rule = field.control.automatic ?? "";
    automatic.subtitle = helpMarkup(
      `${rows.number(automaticMinCores(cores()))} cores: ${rule}`,
      null,
    );
    number.row.visible = !field.automatic;
    number.sync();
  };
}

/**
 * Test a command (DESIGN.md D6): the line as the hook would treat it, with the rules on screen
 * or a draft's, and the rules it matched.
 *
 * @param {TableContext} context
 * @param {Adw.PreferencesGroup} group
 * @param {() => { changes: Change[], rules: HeavyRule[] } | null} proposal What to test
 *   against: the saved rules give no changes; `null` while a draft is not ready.
 * @param {(test: CommandTest | null) => void} onResult
 * @returns {{ run: () => void, cancel: () => void }}
 */
export function commandTestRows(context, group, proposal, onResult) {
  const entry = new Adw.EntryRow({ title: "Command line" });
  const result = dimLabel();
  result.visible = false;
  group.add(entry);
  group.add(result);
  /**
   * @param {string[]} lines
   * @param {CommandTest | null} test
   */
  const show = (lines, test) => {
    result.label = lines.join("\n");
    result.visible = lines.length > 0;
    onResult(test);
  };
  const test = debounced(context.timers, async () => {
    const line = entry.text.trim();
    const against = proposal();
    if (line === "") {
      show([], null);
    } else if (against === null) {
      show([DRAFT_NOT_READY_TEXT], null);
    } else if (context.session.status !== "ready") {
      show([SERVICE_STOPPED_TEXT], null);
    } else {
      const reply = await ask(() => context.session.classify(line, "bash", against.changes));
      if (reply === null) {
        return;
      }
      if (reply.ok) {
        const label = classLabelOf(context.session);
        show(testLines(reply.value, against.rules, label, context.number), reply.value);
      } else {
        show([reply.failure.message], null);
      }
    }
  });
  entry.connect("changed", () => test.run());
  return { run: () => test.run(), cancel: () => test.cancel() };
}

/**
 * The heavy-command rules: one row per rule with its menu, Alt+Up, Alt+Down and Delete, then
 * Add Rule… and Use Built-in Rules….
 *
 * @param {TableContext} context
 * @param {Adw.PreferencesGroup} group
 * @param {RuleListModel} rules
 * @returns {{ sync: Sync, setMatched: (matched: number[]) => void }}
 */
export function ruleListRows(context, group, rules) {
  /** @type {Gtk.Widget[]} */
  let shown = [];
  let signature = "";
  /** @type {number[]} */
  let matched = [];
  /** The rule to focus after the list is rebuilt, such as one just moved. */
  /** @type {number | null} */
  let focus = null;

  const removeAt = (/** @type {number} */ index) => {
    const old = rules.value.slice();
    rules.remove(index);
    focus = Math.min(index, rules.value.length - 1);
    context.toaster.show(ruleDeletedText(index), () => rules.replace(old));
  };
  const move = (/** @type {number} */ from, /** @type {number} */ to) => {
    if (to < 0 || to >= rules.value.length) {
      return;
    }
    focus = to;
    rules.move(from, to);
  };
  const edit = (/** @type {number} */ index) => {
    const draft = rules.edit(index);
    if (draft !== null) {
      openRuleDialog(context, rules, draft);
    }
  };

  const sync = () => {
    group.description = rulesStatus(rules);
    const readOnly = context.readOnly();
    const next = JSON.stringify([rules.value, matched, readOnly]);
    if (next === signature) {
      return;
    }
    signature = next;
    for (const widget of shown) {
      group.remove(widget);
    }
    const label = classLabelOf(context.session);
    /** @type {Gtk.Widget[]} */
    const widgets = rules.value.map((rule, index) => {
      const row = ruleRow(rule, index, rules.value.length, label, matched.includes(index), {
        edit: () => edit(index),
        up: () => move(index, index - 1),
        down: () => move(index, index + 1),
        remove: () => removeAt(index),
      });
      row.sensitive = !readOnly;
      return row;
    });
    if (rules.value.length === 0) {
      const empty = new Adw.ActionRow({ title: escaped(NO_RULES_TEXT), title_lines: 0 });
      widgets.push(empty);
    }
    const add = new Adw.ButtonRow({
      title: "Add Rule…",
      start_icon_name: "list-add-symbolic",
      sensitive: !readOnly,
    });
    add.connect("activated", () => openRuleDialog(context, rules, rules.add()));
    const builtIn = new Adw.ButtonRow({
      title: "Use Built-in Rules…",
      sensitive: !readOnly && rules.canReset,
    });
    builtIn.connect("activated", () =>
      confirm(
        context.parent(),
        useBuiltInConfirmation(rules.value.length, rules.control.default.length),
        () => rules.reset(),
      ));
    widgets.push(add, builtIn);
    for (const widget of widgets) {
      group.add(widget);
    }
    shown = widgets;
    const target = focus === null ? undefined : widgets[focus];
    focus = null;
    target?.grab_focus();
  };

  return {
    sync,
    setMatched: (next) => {
      matched = next;
      sync();
    },
  };
}

/**
 * @param {HeavyRule} rule
 * @param {number} index
 * @param {number} count
 * @param {(jobClass: JobClass) => string} classLabel
 * @param {boolean} matches
 * @param {{ edit: () => void, up: () => void, down: () => void, remove: () => void }} actions
 * @returns {Adw.ActionRow}
 */
function ruleRow(rule, index, count, classLabel, matches, actions) {
  const row = new Adw.ActionRow({
    title: escaped(`${index + 1}. ${rule.program}`),
    subtitle: escaped(`${ruleArguments(rule)} → ${classLabel(rule.class)}`),
    activatable: true,
  });
  row.update_property([Gtk.AccessibleProperty.LABEL], [ruleRowName(index, rule, classLabel)]);
  row.connect("activated", actions.edit);
  if (matches) {
    const mark = new Gtk.Label({ label: "Matches", valign: Gtk.Align.CENTER });
    mark.add_css_class("accent");
    row.add_suffix(mark);
  }

  const group = new Gio.SimpleActionGroup();
  /**
   * @param {string} name
   * @param {() => void} run
   * @param {boolean} [enabled]
   */
  const action = (name, run, enabled = true) => {
    const item = new Gio.SimpleAction({ name, enabled });
    item.connect("activate", run);
    group.add_action(item);
  };
  action("edit", actions.edit);
  action("up", actions.up, index > 0);
  action("down", actions.down, index < count - 1);
  action("delete", actions.remove);
  row.insert_action_group("rule", group);
  const menu = new Gio.Menu();
  menu.append("Edit…", "rule.edit");
  menu.append("Move Up", "rule.up");
  menu.append("Move Down", "rule.down");
  menu.append("Delete", "rule.delete");
  const button = new Gtk.MenuButton({
    icon_name: "view-more-symbolic",
    menu_model: menu,
    valign: Gtk.Align.CENTER,
    tooltip_text: ruleMenuName(index),
  });
  button.add_css_class("flat");
  button.update_property([Gtk.AccessibleProperty.LABEL], [ruleMenuName(index)]);
  row.add_suffix(button);

  const keys = new Gtk.EventControllerKey();
  keys.connect("key-pressed", (_controller, keyval, _keycode, state) => {
    const alt = (state & Gdk.ModifierType.ALT_MASK) !== 0;
    if (alt && keyval === Gdk.KEY_Up) {
      actions.up();
      return true;
    }
    if (alt && keyval === Gdk.KEY_Down) {
      actions.down();
      return true;
    }
    if (!alt && keyval === Gdk.KEY_Delete) {
      actions.remove();
      return true;
    }
    return false;
  });
  row.add_controller(keys);
  return row;
}

/**
 * The dialog of one rule, with its own Test a command box that tests the draft before Save.
 *
 * @param {TableContext} context
 * @param {RuleListModel} rules
 * @param {RuleDraft} draft
 */
function openRuleDialog(context, rules, draft) {
  const { dialog, page, save } = dialogShell(ruleDialogTitle(draft));
  /** @type {Sync[]} */
  const syncs = [];
  const sync = () => {
    for (const part of syncs) {
      part();
    }
  };
  /** @type {RowContext} */
  const rows = {
    number: context.number,
    readOnly: () => false,
    toaster: context.toaster,
    changed: sync,
    noResets: true,
  };

  const programGroup = new Adw.PreferencesGroup();
  const program = new Adw.EntryRow({
    title: escaped(draft.programSpec.label),
    text: draft.program,
  });
  program.connect("changed", () => draft.setProgram(program.text));
  const programHelp = dimLabel();
  programHelp.margin_top = 6;
  programGroup.add(program);
  programGroup.add(programHelp);
  page.add(programGroup);
  syncs.push(() => {
    const issue = draft.programIssue === null ? null : issueText(draft.programIssue, rows.number);
    const help = draft.programSpec.help;
    programHelp.label = issue === null ? help : `${help}\n${issue}`;
  });

  const argsGroup = new Adw.PreferencesGroup({
    title: escaped(draft.args.label),
    description: draft.args.spec.help,
  });
  syncs.push(chipsRows(rows, argsGroup, draft.args, false));
  page.add(argsGroup);

  const classGroup = new Adw.PreferencesGroup();
  const options = draft.classOptions;
  const classRow = new Adw.ComboRow({
    title: escaped(draft.classSpec.label),
    subtitle: escaped(draft.classSpec.help),
    model: Gtk.StringList.new(options.map((option) => option.label)),
  });
  const chosen = options.findIndex((option) => option.value === draft.ruleClass);
  classRow.selected = chosen < 0 ? Gtk.INVALID_LIST_POSITION : chosen;
  classRow.connect("notify::selected", () => {
    const option = options[classRow.selected];
    if (option !== undefined && option.value !== draft.ruleClass) {
      draft.setClass(option.value);
    }
  });
  classGroup.add(classRow);
  page.add(classGroup);

  const testGroup = new Adw.PreferencesGroup({
    title: "Test a Command",
    description: "Shows how the hook would treat this command with this rule.",
  });
  const tester = commandTestRows(
    context,
    testGroup,
    () => {
      const changes = rules.proposal(draft);
      const set = changes?.[0];
      return set?.kind === "set_rules" ? { changes: changes ?? [], rules: set.rules } : null;
    },
    () => {},
  );
  page.add(testGroup);

  draft.onChange = () => {
    sync();
    tester.run();
  };
  save.connect("clicked", () => {
    if (rules.save(draft)) {
      dialog.close();
    } else {
      sync();
    }
  });
  dialog.connect("closed", () => {
    draft.onChange = null;
    tester.cancel();
  });
  dialog.focus_widget = program;
  sync();
  dialog.present(context.parent());
}

/**
 * A dialog with Cancel and Save in its header bar and a preferences page as its body. Escape
 * and Cancel drop the changes.
 *
 * @param {string} title
 * @returns {{ dialog: Adw.Dialog, page: Adw.PreferencesPage, save: Gtk.Button }}
 */
function dialogShell(title) {
  const dialog = new Adw.Dialog({ title, content_width: DIALOG_WIDTH });
  const header = new Adw.HeaderBar({
    show_start_title_buttons: false,
    show_end_title_buttons: false,
  });
  const cancel = new Gtk.Button({ label: "Cancel" });
  cancel.connect("clicked", () => dialog.close());
  const save = new Gtk.Button({ label: "Save" });
  save.add_css_class("suggested-action");
  header.pack_start(cancel);
  header.pack_end(save);
  const view = new Adw.ToolbarView();
  view.add_top_bar(header);
  const page = new Adw.PreferencesPage();
  view.content = page;
  dialog.child = view;
  return { dialog, page, save };
}

/**
 * Names a class from the Classes table's spec, such as "CPU heavy".
 *
 * @param {SettingsSession} session
 * @returns {(jobClass: JobClass) => string}
 */
function classLabelOf(session) {
  return classLabels(session.setting("classes")?.spec.control);
}

/**
 * Runs `work` once the user has paused for the save delay.
 *
 * @param {Timers} timers
 * @param {() => Promise<void>} work
 * @returns {{ run: () => void, cancel: () => void }}
 */
function debounced(timers, work) {
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
        work().catch((error) => console.warn(`Allot: ${error}`));
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
async function ask(request) {
  try {
    return await request();
  } catch (error) {
    if (error instanceof CancelledError) {
      return null;
    }
    throw error;
  }
}

