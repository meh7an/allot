import Adw from "gi://Adw";
import Gio from "gi://Gio";
import GLib from "gi://GLib";
import Gtk from "gi://Gtk";

import { ExtensionPreferences } from "resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js";

import {
  SoupTransport,
  TOKEN_PATH,
  readPanelSettings,
  readTextFile,
  spikeLog,
  writePanelSettings,
} from "./io.js";
import { SPIKE_FIELDS, SettingsClient, changedValues } from "./settings-client.mjs";

/** @import { Field, Outcome, Settings } from "./settings-client.mjs" */
/** @import { PanelSettings } from "./io.js" */

// Edits are sent this long after the last change, so a spin button's clicks make one write.
const APPLY_DELAY_MS = 600;
const DEFAULT_WIDTH = 720;
const DEFAULT_HEIGHT = 640;

// Counts the windows this module has filled; it starts again whenever the prefs service is
// restarted, which shows when GJS reused the module.
let opensInProcess = 0;

const PAGES = [
  { name: "admission", title: "Admission", icon: "preferences-system-symbolic" },
  { name: "leases", title: "Leases and History", icon: "document-open-recent-symbolic" },
  { name: "service", title: "Service", icon: "system-run-symbolic" },
];


export default class SettingsSpikePreferences extends ExtensionPreferences {
  /** @param {Adw.PreferencesWindow} window */
  async fillPreferencesWindow(window) {
    opensInProcess += 1;
    const started = GLib.get_monotonic_time();
    const panel = await readPanelSettings();
    spikeLog("prefs", "fill", {
      opensInProcess,
      pid: Gio.Credentials.new().get_unix_pid(),
      awaitLoad: panel.await_load,
    });
    window.set_default_size(DEFAULT_WIDTH, DEFAULT_HEIGHT);
    window.search_enabled = true;

    const transport = new SoupTransport();
    const client = new SettingsClient({
      send: transport.send,
      readToken: () => readTextFile(TOKEN_PATH),
    });
    const form = new AllotdForm(window, client, panel.spin_rows);
    for (const page of PAGES) {
      window.add(form.page(page));
    }
    window.add(panelPage(panel));
    window.add(spikePage(window, started));

    window.connect("close-request", () => {
      form.close();
      spikeLog("prefs", "close-request");
      return false;
    });
    window.connect("notify::is-active", () =>
      spikeLog("prefs", "active", { active: window.is_active }),
    );
    const load = form.reload().then(() =>
      spikeLog("prefs", "loaded", { ms: elapsedMs(started), status: form.status }),
    );
    if (panel.await_load) {
      await load;
    }
    spikeLog("prefs", "filled", { ms: elapsedMs(started) });
  }
}

/**
 * The allotd pages: shows the values allotd has, and sends each burst of edits as one write
 * checked against the revision on screen (the instant-apply model libadwaita expects).
 */
class AllotdForm {
  /**
   * The control holds the value: the row itself, or a spin button at the row's end.
   *
   * @type {Map<string, {
   *   field: Field,
   *   row: Adw.ActionRow,
   *   control: Adw.SpinRow | Adw.SwitchRow | Gtk.SpinButton,
   *   reset: Gtk.Button,
   * }>}
   */
  #rows = new Map();
  /** @type {Adw.PreferencesGroup[]} */
  #groups = [];
  /** @type {Adw.ActionRow | null} */
  #statusRow = null;
  /** @type {Adw.Toast | null} */
  #lastToast = null;
  /** @type {Settings | null} */
  #settings = null;
  /** @type {Record<string, number | boolean>} */
  #pending = {};
  #timer = 0;
  #sending = false;
  // Set while values from allotd are put on screen, so they are not sent back as edits.
  #showing = false;
  #window;
  #client;
  #spinRows;
  status = "loading";

  /**
   * @param {Adw.PreferencesWindow} window
   * @param {SettingsClient} client
   * @param {boolean} spinRows Numbers as `Adw.SpinRow`, else as an action row with a
   *   `Gtk.SpinButton`, to compare what each shows to assistive technology.
   */
  constructor(window, client, spinRows) {
    this.#window = window;
    this.#client = client;
    this.#spinRows = spinRows;
  }

  /**
   * @param {{ name: string, title: string, icon: string }} spec
   * @returns {Adw.PreferencesPage}
   */
  page(spec) {
    const page = new Adw.PreferencesPage({ title: spec.title, icon_name: spec.icon });
    if (spec.name === "admission") {
      page.add(this.#statusGroup());
    }
    const group = new Adw.PreferencesGroup({ title: spec.title, sensitive: false });
    for (const field of SPIKE_FIELDS.filter((candidate) => candidate.page === spec.name)) {
      group.add(this.#row(field));
    }
    this.#groups.push(group);
    page.add(group);
    return page;
  }

  /** @returns {Promise<void>} */
  async reload() {
    const outcome = await this.#client.load();
    spikeLog("prefs", "load", { status: outcome.status, detail: outcome.detail });
    this.#show(outcome, null);
  }

  /** Sends what is still waiting, so a change made just before closing is not lost. */
  close() {
    if (this.#timer !== 0) {
      GLib.source_remove(this.#timer);
      this.#timer = 0;
      this.#send();
    }
  }

  /** @returns {Adw.PreferencesGroup} */
  #statusGroup() {
    const group = new Adw.PreferencesGroup({ title: "allotd" });
    this.#statusRow = new Adw.ActionRow({ title: "Connecting...", subtitle: "" });
    const retry = new Gtk.Button({ label: "Retry", valign: Gtk.Align.CENTER });
    retry.connect("clicked", () => this.reload());
    this.#statusRow.add_suffix(retry);
    group.add(this.#statusRow);
    return group;
  }

  /**
   * @param {Field} field
   * @returns {Adw.PreferencesRow}
   */
  #row(field) {
    const reset = new Gtk.Button({
      icon_name: "edit-undo-symbolic",
      tooltip_text: "Reset to default",
      valign: Gtk.Align.CENTER,
      visible: false,
    });
    reset.add_css_class("flat");
    reset.connect("clicked", () => this.#reset(field.key));
    const titles = { title: field.title, subtitle: field.subtitle };
    let row;
    let control;
    if (field.kind === "switch") {
      row = control = new Adw.SwitchRow(titles);
      control.connect("notify::active", () => this.#edit(field.key, control.active));
    } else {
      const adjustment = new Gtk.Adjustment({
        lower: field.min ?? 0,
        upper: field.max ?? 100,
        step_increment: field.step ?? 1,
      });
      if (this.#spinRows) {
        row = control = new Adw.SpinRow({ ...titles, adjustment });
      } else {
        control = new Gtk.SpinButton({ adjustment, valign: Gtk.Align.CENTER });
        row = new Adw.ActionRow({ ...titles, activatable_widget: control });
        row.add_suffix(control);
      }
      control.connect("notify::value", () => this.#edit(field.key, control.value));
    }
    row.add_suffix(reset);
    this.#rows.set(field.key, { field, row, control, reset });
    return row;
  }

  /**
   * @param {string} key
   * @param {number | boolean} value
   */
  #edit(key, value) {
    if (this.#showing) {
      return;
    }
    this.#pending[key] = value;
    this.#schedule();
  }

  #schedule() {
    if (this.#timer !== 0) {
      GLib.source_remove(this.#timer);
    }
    this.#timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, APPLY_DELAY_MS, () => {
      this.#timer = 0;
      this.#send();
      return GLib.SOURCE_REMOVE;
    });
  }

  async #send() {
    if (this.#settings === null) {
      return;
    }
    if (this.#sending) {
      // One write at a time; the next goes once this one has a revision to build on.
      this.#schedule();
      return;
    }
    const mine = changedValues(this.#settings, this.#pending);
    this.#pending = {};
    if (Object.keys(mine).length === 0) {
      return;
    }
    await this.#write(mine, []);
  }

  /** @param {string} key */
  async #reset(key) {
    if (this.#settings !== null) {
      await this.#write({}, [key]);
    }
  }

  /**
   * @param {Record<string, number | boolean>} set
   * @param {string[]} reset
   */
  async #write(set, reset) {
    const started = GLib.get_monotonic_time();
    this.#sending = true;
    const outcome = await this.#client.apply(
      /** @type {Settings} */ (this.#settings).revision,
      set,
      reset,
    );
    this.#sending = false;
    spikeLog("prefs", "write", {
      set,
      reset,
      status: outcome.status,
      ms: elapsedMs(started),
      revision: outcome.settings?.revision ?? null,
    });
    this.#show(outcome, set);
  }

  /**
   * @param {Outcome} outcome
   * @param {Record<string, number | boolean> | null} mine The edits a write sent; `null` for a
   *   load.
   */
  #show(outcome, mine) {
    this.status = outcome.status;
    switch (outcome.status) {
      case "ok":
        this.#showConnected(/** @type {Settings} */ (outcome.settings));
        break;
      case "conflict":
        this.#showConnected(/** @type {Settings} */ (outcome.settings));
        this.#toast("Settings changed outside this window", "Apply Mine", () =>
          this.#write(mine ?? {}, []),
        );
        break;
      case "invalid":
        this.#toast(outcome.errors.map((error) => error.message).join("; "));
        this.#showValues(/** @type {Settings} */ (this.#settings));
        break;
      default:
        this.#showDown(outcome.detail);
        if (mine !== null && this.#settings !== null) {
          this.#toast(`Not saved: ${outcome.detail}`);
          this.#showValues(this.#settings);
        }
    }
  }

  /** @param {Settings} settings */
  #showConnected(settings) {
    this.#showValues(settings);
    for (const group of this.#groups) {
      group.sensitive = true;
    }
    this.#statusRow?.set_title("Connected");
    this.#statusRow?.set_subtitle(`Revision ${settings.revision}`);
  }

  /** @param {Settings} settings */
  #showValues(settings) {
    this.#settings = settings;
    this.#showing = true;
    for (const { field, row, control, reset } of this.#rows.values()) {
      const value = settings.values[field.key];
      if (control instanceof Adw.SwitchRow) {
        control.active = Boolean(value);
      } else {
        control.value = Number(value);
      }
      reset.visible = value !== settings.defaults[field.key];
      if (field.restart) {
        row.set_subtitle(
          settings.restart_pending
            ? "Restart allotd to use the new port."
            : field.subtitle,
        );
      }
    }
    this.#showing = false;
  }

  /**
   * Values on screen may be out of date, so they cannot be edited until allotd answers again.
   *
   * @param {string} detail
   */
  #showDown(detail) {
    for (const group of this.#groups) {
      group.sensitive = false;
    }
    this.#statusRow?.set_title("allotd is not answering");
    this.#statusRow?.set_subtitle(detail);
  }

  /**
   * @param {string} title
   * @param {string} [button]
   * @param {() => void} [onButton]
   */
  #toast(title, button, onButton) {
    // Toasts queue, so without this a conflict could wait behind an older message.
    this.#lastToast?.dismiss();
    const toast = new Adw.Toast({ title, timeout: 6 });
    if (button !== undefined && onButton !== undefined) {
      toast.button_label = button;
      toast.connect("button-clicked", onButton);
    }
    this.#lastToast = toast;
    this.#window.add_toast(toast);
  }
}

/**
 * The panel's own settings, written to its file as the real panel writes `tray.json`.
 *
 * @param {PanelSettings} initial
 * @returns {Adw.PreferencesPage}
 */
function panelPage(initial) {
  const page = new Adw.PreferencesPage({ title: "Panel", icon_name: "view-grid-symbolic" });
  const group = new Adw.PreferencesGroup({
    title: "Panel",
    description:
      "Saved by this window; the panel menu shows Show CPU as it changes. The last two take " +
      "effect on the next open.",
  });
  const settings = { ...initial };
  /**
   * @param {keyof PanelSettings} key
   * @param {string} title
   * @param {string} subtitle
   */
  const toggle = (key, title, subtitle) => {
    const row = new Adw.SwitchRow({ title, subtitle, active: settings[key] });
    row.connect("notify::active", () => {
      settings[key] = row.active;
      writePanelSettings(settings).catch((error) =>
        spikeLog("prefs", "panel-write-failed", { error: String(error) }),
      );
    });
    group.add(row);
  };
  toggle("show_cpu", "Show CPU", "A stand-in for the meter toggles.");
  toggle("spin_rows", "Numbers as spin rows", "Off: an action row with a spin button.");
  toggle(
    "await_load",
    "Wait for allotd before showing",
    "Try it with the slow mode.",
  );
  page.add(group);
  return page;
}

/**
 * What the spike measures, and the two ways of closing the window.
 *
 * @param {Adw.PreferencesWindow} window
 * @param {number} started
 * @returns {Adw.PreferencesPage}
 */
function spikePage(window, started) {
  const page = new Adw.PreferencesPage({ title: "Spike", icon_name: "dialog-information-symbolic" });
  const group = new Adw.PreferencesGroup({ title: "This process" });
  /**
   * @param {string} title
   * @param {string} subtitle
   */
  const line = (title, subtitle) => group.add(new Adw.ActionRow({ title, subtitle }));
  line("Process", `${Gio.Credentials.new().get_unix_pid()}`);
  line("Windows filled by this module", `${opensInProcess}`);
  line("Filled at", `${elapsedMs(started)} ms after the call`);
  /**
   * @param {string} label
   * @param {() => void} onClick
   */
  const button = (label, onClick) => {
    const row = new Adw.ActionRow({ title: label });
    const widget = new Gtk.Button({ label: "Run", valign: Gtk.Align.CENTER });
    widget.connect("clicked", onClick);
    row.add_suffix(widget);
    group.add(row);
  };
  button("Close with close()", () => window.close());
  button("Close with destroy() (the shell says this shows an error page)", () => window.destroy());
  page.add(group);
  return page;
}

/**
 * @param {number} started A `GLib.get_monotonic_time()` reading.
 * @returns {number}
 */
function elapsedMs(started) {
  return Math.round((GLib.get_monotonic_time() - started) / 1000);
}
