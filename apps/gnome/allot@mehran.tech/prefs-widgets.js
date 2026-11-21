import Adw from "gi://Adw";
import Gdk from "gi://Gdk";
import GLib from "gi://GLib";
import Gtk from "gi://Gtk";

import { resetField } from "./lib/field-reset.js";
import { bannerActionLabel } from "./lib/settings-banners.js";
import {
  DurationModel,
  NumberModel,
  StringListModel,
  SwitchModel,
  issueText,
} from "./lib/setting-values.js";
import { fieldResetText, resetButtonName } from "./lib/settings-text.js";

/**
 * @import { BannerAction, SettingsBanner } from "./lib/settings-banners.js"
 * @import { Issue, ValueModel } from "./lib/setting-values.js"
 * @import { Confirmation } from "./lib/settings-text.js"
 */

// The rows of the Settings window that every page and dialog shares (DESIGN.md section 3, and
// section 10's GNOME column): a setting's row with its help, key, issue line and reset button,
// number fields with their unit, chips, banners, toasts and confirmations. This file runs in the
// preferences process, so it imports GTK and libadwaita only, never the shell's modules.

/**
 * What the rows need from the window around them.
 *
 * @typedef {object} RowContext
 * @property {(value: number) => string} number Formats a number for the user's locale.
 * @property {() => boolean} readOnly Whether allotd's settings cannot be changed now.
 * @property {Toaster} toaster
 * @property {() => void} changed Something on screen changed without the session noticing,
 *   such as a unit picked, so the window shows it again.
 * @property {boolean} [noResets] Hides the reset buttons, as in an edit dialog, where a field
 *   has no default of its own.
 */

/** Synchronises a part of the window with its view models. */
/** @typedef {() => void} Sync */

const RANGE_LIMIT = 1e9;
const UNDO_TOAST_SECS = 10;
const NOTICE_TOAST_SECS = 5;

/**
 * Text for a label or row that must not be read as Pango markup.
 *
 * @param {string} text
 * @returns {string}
 */
export function escaped(text) {
  return GLib.markup_escape_text(text, -1);
}

/**
 * A row's subtitle: the help, the key in monospace (DESIGN.md D4), then lines such as an issue
 * or a disabled note.
 *
 * @param {string} help
 * @param {string | null} key
 * @param {Array<string | null>} [notes]
 * @returns {string} Pango markup.
 */
export function helpMarkup(help, key, notes = []) {
  const keyText = key === null ? "" : `  <small><tt>${escaped(key)}</tt></small>`;
  const extra = notes.filter((note) => note !== null && note !== "").map((note) => escaped(note));
  return [`${escaped(help)}${keyText}`, ...extra].join("\n");
}

/**
 * The same help, key and notes as plain text, for a control's accessible description.
 *
 * @param {string} help
 * @param {string | null} key
 * @param {Array<string | null>} [notes]
 * @returns {string}
 */
export function helpText(help, key, notes = []) {
  const first = key === null ? help : `${help} Key: ${key}.`;
  return [first, ...notes.filter((note) => note !== null && note !== "")].join(" ");
}

/** What each control was last given, so unchanged text is never set again. */
const described = new WeakMap();

/**
 * Gives a control its description (the help, the key and any error) and its invalid state. Orca
 * reads out every change of the focused control's description, so this sets only what changed and
 * never takes text that ticks.
 *
 * @param {Gtk.Widget} widget
 * @param {string} text
 * @param {boolean} [invalid] Whether the control has an error now.
 */
export function describe(widget, text, invalid = false) {
  const signature = `${invalid ? "1" : "0"}${text}`;
  if (described.get(widget) === signature) {
    return;
  }
  described.set(widget, signature);
  widget.update_property([Gtk.AccessibleProperty.DESCRIPTION], [text]);
  widget.update_state(
    [Gtk.AccessibleState.INVALID],
    [invalid ? Gtk.AccessibleInvalidState.TRUE : Gtk.AccessibleInvalidState.FALSE],
  );
}

/**
 * The widget of an entry row that takes focus and reads as the text field.
 *
 * @param {Adw.EntryRow} row
 * @returns {Gtk.Widget}
 */
export function innerEntry(row) {
  return row.get_delegate() ?? row;
}

/** One toast at a time: toasts queue, so the previous one is dismissed first (M0.3). */
export class Toaster {
  #target;
  /** @type {Adw.Toast | null} */
  #last = null;

  /** @param {{ add_toast: (toast: Adw.Toast) => void }} target */
  constructor(target) {
    this.#target = target;
  }

  /**
   * @param {string} title
   * @param {(() => void) | null} [undo] Adds an Undo button.
   */
  show(title, undo = null) {
    this.#last?.dismiss();
    const toast = new Adw.Toast({
      title: escaped(title),
      timeout: undo === null ? NOTICE_TOAST_SECS : UNDO_TOAST_SECS,
    });
    if (undo !== null) {
      toast.button_label = "Undo";
      toast.connect("button-clicked", () => undo());
    }
    this.#last = toast;
    this.#target.add_toast(toast);
  }
}

/**
 * Asks before something that cannot be undone; the primary button is destructive.
 *
 * @param {Gtk.Widget} parent
 * @param {Confirmation} confirmation
 * @param {() => void} onConfirm
 */
export function confirm(parent, confirmation, onConfirm) {
  const dialog = new Adw.AlertDialog({
    heading: confirmation.title,
    body: confirmation.body,
  });
  dialog.add_response("cancel", "Cancel");
  dialog.add_response("confirm", confirmation.button);
  dialog.set_response_appearance("confirm", Adw.ResponseAppearance.DESTRUCTIVE);
  dialog.default_response = "cancel";
  dialog.close_response = "cancel";
  dialog.connect("response", (_dialog, response) => {
    if (response === "confirm") {
      onConfirm();
    }
  });
  dialog.present(parent);
}

/**
 * A flat icon button with an accessible name.
 *
 * @param {string} iconName
 * @param {string} name
 * @param {() => void} onClick
 * @returns {Gtk.Button}
 */
export function iconButton(iconName, name, onClick) {
  const button = new Gtk.Button({
    icon_name: iconName,
    tooltip_text: name,
    valign: Gtk.Align.CENTER,
  });
  button.add_css_class("flat");
  button.update_property([Gtk.AccessibleProperty.LABEL], [name]);
  button.connect("clicked", onClick);
  return button;
}

/**
 * A text button in a row's suffix.
 *
 * @param {string} label
 * @param {() => void} onClick
 * @returns {Gtk.Button}
 */
export function textButton(label, onClick) {
  const button = new Gtk.Button({ label, valign: Gtk.Align.CENTER });
  button.connect("clicked", onClick);
  return button;
}

/**
 * A wrapping secondary line inside a group, such as a live sentence or a test result.
 *
 * @returns {Gtk.Label}
 */
export function dimLabel() {
  const label = new Gtk.Label({
    wrap: true,
    xalign: 0,
    margin_top: 12,
    selectable: true,
  });
  label.add_css_class("dim-label");
  return label;
}

/**
 * A line of help under an entry that shows an error with an icon as well as text, so an error is
 * never told by colour alone.
 */
export class HelpLine {
  /** @type {Gtk.Box} */
  widget;
  #icon;
  #label;
  #issueOnly;

  /** @param {boolean} [issueOnly] Whether the line is hidden until it has an error. */
  constructor(issueOnly = false) {
    this.#issueOnly = issueOnly;
    this.widget = new Gtk.Box({ spacing: 8, margin_top: 6 });
    this.#icon = new Gtk.Image({
      icon_name: "dialog-error-symbolic",
      visible: false,
      valign: Gtk.Align.START,
    });
    this.#icon.update_property([Gtk.AccessibleProperty.LABEL], ["Error"]);
    this.#label = new Gtk.Label({ wrap: true, xalign: 0, hexpand: true, selectable: true });
    this.#label.add_css_class("dim-label");
    this.widget.append(this.#icon);
    this.widget.append(this.#label);
  }

  /**
   * @param {string} help The line that always shows, empty for none.
   * @param {string | null} issue The error, if any.
   */
  show(help, issue) {
    this.#label.label = issue === null || help === "" ? (issue ?? help) : `${help}
${issue}`;
    this.#icon.visible = issue !== null;
    this.widget.visible = !this.#issueOnly || issue !== null;
  }
}

/**
 * The field's reset button: shown only while the saved value differs from the default. The
 * notice's Undo writes the old value back.
 *
 * @param {RowContext} context
 * @param {ValueModel<unknown>} model
 * @returns {{ button: Gtk.Button, sync: Sync }}
 */
function resetButton(context, model) {
  const button = iconButton("edit-undo-symbolic", resetButtonName(model, context.number), () => {
    const text = fieldResetText(model, context.number);
    const undo = resetField(model);
    context.toaster.show(text, undo);
  });
  return {
    button,
    sync: () => {
      const name = resetButtonName(model, context.number);
      button.visible = !context.noResets && model.canReset && !context.readOnly();
      button.tooltip_text = name;
      button.update_property([Gtk.AccessibleProperty.LABEL], [name]);
    },
  };
}

/**
 * An icon in front of a row that shows while the row has an issue, so an error is never told by
 * colour alone.
 *
 * @returns {Gtk.Image}
 */
function issueIcon() {
  const icon = new Gtk.Image({ icon_name: "dialog-error-symbolic", visible: false });
  icon.update_property([Gtk.AccessibleProperty.LABEL], ["Error"]);
  return icon;
}

/**
 * @param {Issue | null} issue
 * @param {(value: number) => string} number
 * @returns {string | null}
 */
function issueLine(issue, number) {
  return issue === null ? null : issueText(issue, number);
}

/**
 * The rows of one setting, added to `group`. Size tables and rule lists have sections of their
 * own (`prefs-tables.js`).
 *
 * @param {RowContext} context
 * @param {Adw.PreferencesGroup} group
 * @param {ValueModel<unknown>} model
 * @param {(key: string) => string | null} [note] An extra line under a row, such as Port's
 *   restart note.
 * @returns {Sync}
 */
export function settingRows(context, group, model, note = () => null) {
  if (model instanceof SwitchModel) {
    return switchRow(context, group, model);
  }
  if (model instanceof DurationModel && model.off !== null) {
    const toggle = offSwitchRow(context, group, model);
    const duration = numberRow(context, model, note);
    group.add(duration.row);
    return () => {
      toggle();
      duration.sync();
    };
  }
  if (model instanceof NumberModel) {
    const number = numberRow(context, model, note);
    group.add(number.row);
    return number.sync;
  }
  if (model instanceof StringListModel) {
    return chipsRows(context, group, model);
  }
  return () => {};
}

/**
 * @param {RowContext} context
 * @param {Adw.PreferencesGroup} group
 * @param {SwitchModel} model
 * @returns {Sync}
 */
function switchRow(context, group, model) {
  const row = new Adw.SwitchRow({ title: escaped(model.label) });
  const reset = resetButton(context, model);
  const icon = issueIcon();
  row.add_prefix(icon);
  row.add_suffix(reset.button);
  let syncing = false;
  row.connect("notify::active", () => {
    if (!syncing && row.active !== model.value) {
      model.set(row.active);
    }
  });
  group.add(row);
  return () => {
    syncing = true;
    row.active = model.value;
    syncing = false;
    row.sensitive = model.enabled && !context.readOnly();
    const issue = issueLine(model.issue, context.number);
    row.subtitle = helpMarkup(model.spec.help, model.key, [model.disabledNote, issue]);
    const help = helpText(model.spec.help, model.key, [model.disabledNote, issue]);
    describe(row, help, issue !== null);
    icon.visible = model.issue !== null;
    reset.sync();
  };
}

/**
 * The switch in front of a duration that one value turns off, such as End leases left idle.
 *
 * @param {RowContext} context
 * @param {Adw.PreferencesGroup} group
 * @param {DurationModel} model
 * @returns {Sync}
 */
function offSwitchRow(context, group, model) {
  const off = /** @type {NonNullable<DurationModel["off"]>} */ (model.off);
  const row = new Adw.SwitchRow({
    title: escaped(off.label),
    subtitle: helpMarkup(off.help, model.key),
  });
  let syncing = false;
  row.connect("notify::active", () => {
    if (!syncing && row.active !== model.isOn) {
      model.setOn(row.active);
    }
  });
  describe(row, helpText(off.help, model.key));
  group.add(row);
  return () => {
    syncing = true;
    row.active = model.isOn;
    syncing = false;
    row.sensitive = model.enabled && !context.readOnly();
  };
}

/**
 * A number with its spin button, the unit picker of a duration or memory size, and its reset
 * button. `Adw.SpinRow` is not used: on GNOME 50 it was missing from the accessibility tree
 * (M0.3, DESIGN.md R7).
 *
 * @param {RowContext} context
 * @param {NumberModel} model
 * @param {(key: string) => string | null} [note]
 * @returns {{ row: Adw.ActionRow, sync: Sync, input: Gtk.Widget }} The input is the spin button,
 *   which a dialog focuses first.
 */
export function numberRow(context, model, note = () => null) {
  const row = new Adw.ActionRow({ title: escaped(model.label) });
  const editor = numberEditor(context, model);
  const reset = resetButton(context, model);
  const icon = issueIcon();
  row.add_prefix(icon);
  for (const widget of editor.widgets) {
    row.add_suffix(widget);
  }
  row.add_suffix(reset.button);
  row.activatable_widget = editor.widgets[0] ?? null;
  // Indented under its switch, as DESIGN.md asks for a duration that can be off.
  // A boxed list has no indented rows, so a spacer in front stands in for one.
  if (model instanceof DurationModel && model.off !== null) {
    row.add_prefix(new Gtk.Box({ width_request: 24 }));
  }
  const sync = () => {
    editor.sync();
    const on = !(model instanceof DurationModel) || model.isOn;
    row.sensitive = model.enabled && on && !context.readOnly();
    const notes = [model.disabledNote, note(model.key), issueLine(model.issue, context.number)];
    row.subtitle = helpMarkup(model.spec.help, model.key, notes);
    editor.describe(helpText(model.spec.help, model.key, notes), model.issue !== null);
    icon.visible = model.issue !== null;
    reset.sync();
  };
  return { row, sync, input: editor.widgets[0] ?? row };
}

/**
 * A spin button for a number, plus a unit drop-down when it has units. It commits on Enter, on
 * leaving the field or on a spin step; Escape shows the saved value again.
 *
 * @param {RowContext} context
 * @param {NumberModel} model
 * @returns {{
 *   widgets: Gtk.Widget[],
 *   sync: Sync,
 *   describe: (text: string, invalid: boolean) => void,
 * }}
 */
export function numberEditor(context, model) {
  const control = model.control;
  const spin = new Gtk.SpinButton({
    adjustment: new Gtk.Adjustment({
      // Wide limits, so a value out of range shows its issue line instead of being clamped.
      lower: -RANGE_LIMIT,
      upper: RANGE_LIMIT,
      step_increment: 1,
      page_increment: 10,
    }),
    digits: control.integer ? 0 : 2,
    numeric: true,
    valign: Gtk.Align.CENTER,
    width_chars: 7,
  });
  spin.update_property([Gtk.AccessibleProperty.LABEL], [model.label]);
  let syncing = false;
  spin.connect("value-changed", () => {
    if (!syncing) {
      model.commit(spin.value);
    }
  });
  const keys = new Gtk.EventControllerKey({ propagation_phase: Gtk.PropagationPhase.CAPTURE });
  keys.connect("key-pressed", (_controller, keyval) => {
    if (keyval !== Gdk.KEY_Escape || !model.dirty) {
      return false;
    }
    model.undo();
    return true;
  });
  spin.add_controller(keys);

  /** @type {Gtk.Widget[]} */
  const widgets = [spin];
  /** @type {Gtk.DropDown | null} */
  let units = null;
  if (model.units.length > 0) {
    const picker = Gtk.DropDown.new_from_strings(model.units.map((unit) => unit.name));
    picker.valign = Gtk.Align.CENTER;
    picker.update_property([Gtk.AccessibleProperty.LABEL], [`${model.label} unit`]);
    picker.connect("notify::selected", () => {
      const unit = model.units[picker.selected];
      if (!syncing && unit !== undefined) {
        model.setUnit(unit.name);
        context.changed();
      }
    });
    widgets.push(picker);
    units = picker;
  }

  return {
    widgets,
    describe: (text, invalid) => {
      for (const widget of widgets) {
        describe(widget, text, invalid);
      }
    },
    sync: () => {
      syncing = true;
      const shown = model.shown ?? 0;
      // Setting an equal value rewrites the text, which would wipe what the user is typing.
      if (Math.abs(spin.value - shown) > 1e-9) {
        spin.value = shown;
      }
      if (units !== null) {
        const index = model.units.findIndex((unit) => unit.name === model.unit.name);
        if (index >= 0 && units.selected !== index) {
          units.selected = index;
        }
      }
      syncing = false;
    },
  };
}

/**
 * A string list as chips with remove buttons, and an entry that adds one (DESIGN.md section 3).
 * Adding or removing saves.
 *
 * @param {RowContext} context
 * @param {Adw.PreferencesGroup} group
 * @param {StringListModel} model
 * @param {boolean} [withReset] Whether the list has a reset button in the group's header.
 * @returns {Sync}
 */
export function chipsRows(context, group, model, withReset = true) {
  const chips = new Adw.WrapBox({
    child_spacing: 6,
    line_spacing: 6,
    margin_top: 8,
    margin_bottom: 8,
    margin_start: 12,
    margin_end: 12,
  });
  // In a row of its own, so the chips sit in the group's list above the entry.
  const chipsRow = new Adw.PreferencesRow({ activatable: false, focusable: false, child: chips });
  const entry = new Adw.EntryRow({
    title: `Add to ${model.label}`,
    show_apply_button: true,
  });
  const issue = new HelpLine(true);
  const add = () => {
    // Enter can both apply and activate the entry; the second call finds it empty.
    if (entry.text.trim() === "") {
      return;
    }
    if (model.add(entry.text)) {
      entry.text = "";
    }
    context.changed();
  };
  entry.connect("apply", add);
  entry.connect("entry-activated", add);
  entry.connect("changed", () => {
    if (model.entryChanged()) {
      context.changed();
    }
  });
  const reset = resetButton(context, model);
  if (withReset) {
    group.header_suffix = reset.button;
    group.description = `${model.spec.help}  ${model.key}`;
  }
  group.add(chipsRow);
  group.add(entry);
  group.add(issue.widget);

  /** @type {Gtk.Widget[]} */
  let shown = [];
  let signature = "";
  return () => {
    const readOnly = context.readOnly();
    const next = JSON.stringify([model.value, readOnly]);
    if (next !== signature) {
      signature = next;
      for (const chip of shown) {
        chips.remove(chip);
      }
      shown = model.value.map((value, index) => {
        const chip = new Gtk.Button({
          child: new Adw.ButtonContent({ label: value, icon_name: "window-close-symbolic" }),
          sensitive: !readOnly,
          tooltip_text: `Remove ${value}`,
        });
        chip.add_css_class("pill");
        chip.update_property([Gtk.AccessibleProperty.LABEL], [`Remove ${value}`]);
        chip.connect("clicked", () => model.remove(index));
        chips.append(chip);
        return chip;
      });
    }
    entry.sensitive = !readOnly;
    const line = issueLine(model.issue, context.number);
    issue.show("", line);
    describe(
      innerEntry(entry),
      helpText(model.spec.help, withReset ? model.key : null, [line]),
      line !== null,
    );
    if (withReset) {
      reset.sync();
    }
  };
}

/**
 * The banners at the top of a page (DESIGN.md section 7). Each is a row with an icon, the
 * message and its buttons, since `Adw.Banner` holds only one button and several states need two.
 *
 * @param {Adw.PreferencesGroup} group
 * @param {(action: BannerAction) => void} onAction
 * @returns {(banners: SettingsBanner[]) => void}
 */
export function bannerRows(group, onAction) {
  /** @type {Gtk.Widget[]} */
  let rows = [];
  let signature = "";
  return (banners) => {
    const next = JSON.stringify(banners);
    if (next === signature) {
      return;
    }
    signature = next;
    for (const row of rows) {
      group.remove(row);
    }
    rows = banners.map((banner) => {
      const row = new Adw.ActionRow({ title: escaped(banner.message), title_lines: 0 });
      const icon = new Gtk.Image({ icon_name: SEVERITY_ICONS[banner.severity] });
      icon.update_property([Gtk.AccessibleProperty.LABEL], [SEVERITY_NAMES[banner.severity]]);
      row.add_prefix(icon);
      banner.actions.forEach((action, index) => {
        const button = textButton(bannerActionLabel(action), () => onAction(action));
        if (index === 0) {
          button.add_css_class("suggested-action");
        }
        row.add_suffix(button);
      });
      group.add(row);
      return row;
    });
  };
}

/** @type {Record<SettingsBanner["severity"], string>} */
const SEVERITY_NAMES = { info: "Information", warning: "Warning", error: "Error" };

/** @type {Record<SettingsBanner["severity"], string>} */
const SEVERITY_ICONS = {
  info: "dialog-information-symbolic",
  warning: "dialog-warning-symbolic",
  error: "dialog-error-symbolic",
};
