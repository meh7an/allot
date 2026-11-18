import Clutter from "gi://Clutter";
import Gio from "gi://Gio";
import GLib from "gi://GLib";
import St from "gi://St";

import { Extension } from "resource:///org/gnome/shell/extensions/extension.js";
import * as Config from "resource:///org/gnome/shell/misc/config.js";
import * as Main from "resource:///org/gnome/shell/ui/main.js";
import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";
import * as PopupMenu from "resource:///org/gnome/shell/ui/popupMenu.js";

import { PANEL_SETTINGS_PATH, readPanelSettings, spikeLog } from "./io.js";

// The gnome-shell service that runs every extension's prefs.js.
const PREFS_APP_ID = "org.gnome.Shell.Extensions";
const DELAYED_OPEN_MS = 3000;
// Long enough for the window to map and for any activation to land.
const FOCUS_PROBE_MS = 800;

/**
 * The stand-in panel button. Its menu opens Settings three ways, so the checklist can compare a
 * plain `openPreferences()` with one that raises a window already open, and shows a panel setting
 * that the prefs process writes, to prove the shell sees it change.
 */
export default class SettingsSpike extends Extension {
  enable() {
    this._timeouts = new Set();
    this._button = new PanelMenu.Button(0.5, this.metadata.name);
    this._button.add_child(new St.Label({ text: "Spike", y_align: Clutter.ActorAlign.CENTER }));
    const menu = /** @type {PopupMenu.PopupMenu} */ (this._button.menu);
    menu.addAction("Settings", () => this.openSettings("menu"));
    menu.addAction("Settings (openPreferences only)", () => this.openPlain("menu"));
    menu.addAction("Settings in 3 s", () =>
      this.#later(DELAYED_OPEN_MS, () => this.openSettings("delayed")),
    );
    menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    this._panelLine = new PopupMenu.PopupMenuItem("Show CPU: reading...", { reactive: false });
    menu.addMenuItem(this._panelLine);
    Main.panel.addToStatusArea(this.uuid, this._button);

    this._windowCreated = global.display.connect("window-created", (_display, window) =>
      this.#noteWindow(window),
    );
    this._monitor = Gio.File.new_for_path(PANEL_SETTINGS_PATH).monitor_file(
      Gio.FileMonitorFlags.WATCH_MOVES,
      null,
    );
    this._monitor.connect("changed", (_monitor, _file, _other, eventType) => {
      spikeLog("shell", "panel-file", { change: eventName(eventType) });
      this.#showPanelSettings();
    });
    this.#showPanelSettings();
    spikeLog("shell", "enabled", { shell: Config.PACKAGE_VERSION });
  }

  disable() {
    this._monitor.cancel();
    global.display.disconnect(this._windowCreated);
    for (const id of this._timeouts) {
      GLib.source_remove(id);
    }
    this._button.destroy();
    this._monitor = null;
    this._button = null;
    this._panelLine = null;
  }

  /**
   * Raises the open Settings window, which `openPreferences()` alone does not do, or opens one.
   *
   * @param {string} from
   */
  openSettings(from) {
    const open = this.#prefsWindow();
    if (open === null) {
      this.openPlain(from);
      return;
    }
    Main.activateWindow(open);
    spikeLog("shell", "raise", { from });
    this.#probeFocus(open, "raise");
  }

  /** @param {string} from */
  openPlain(from) {
    const returned = this.openPreferences();
    spikeLog("shell", "open-preferences", {
      from,
      returned: String(returned),
      open: this.#prefsWindow() !== null,
    });
  }

  /** @returns {import("gi://Meta").default.Window | null} */
  #prefsWindow() {
    const windows = global.get_window_actors().map((actor) => actor.meta_window);
    return windows.find((window) => this.#isSettings(window)) ?? null;
  }

  /** @param {import("gi://Meta").default.Window} window */
  #isSettings(window) {
    return isPrefsService(window) && window.get_title() === this.metadata.name;
  }

  /** @param {import("gi://Meta").default.Window} window */
  #noteWindow(window) {
    // A new window has no class or title yet, so both are read once it has settled.
    this.#later(FOCUS_PROBE_MS, () => {
      if (isPrefsService(window)) {
        this.#reportWindow(window, "created");
      }
    });
  }

  /**
   * @param {import("gi://Meta").default.Window} window
   * @param {string} why
   */
  #probeFocus(window, why) {
    this.#later(FOCUS_PROBE_MS, () => this.#reportWindow(window, why));
  }

  /**
   * @param {import("gi://Meta").default.Window} window
   * @param {string} why
   */
  #reportWindow(window, why) {
    const focus = global.display.focus_window;
    const stack = global.display.sort_windows_by_stacking([window, ...othersOf(window)]);
    spikeLog("shell", "settings-window", {
      why,
      title: window.get_title(),
      wmClass: window.get_wm_class(),
      gtkAppId: window.get_gtk_application_id(),
      focused: window.has_focus(),
      focusTitle: focus?.get_title() ?? null,
      onTop: stack.at(-1) === window,
      width: window.get_frame_rect().width,
      height: window.get_frame_rect().height,
    });
  }

  async #showPanelSettings() {
    const settings = await readPanelSettings();
    this._panelLine?.label.set_text(`Show CPU: ${settings.show_cpu ? "on" : "off"}`);
  }

  /**
   * @param {number} ms
   * @param {() => void} callback
   */
  #later(ms, callback) {
    const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
      this._timeouts.delete(id);
      callback();
      return GLib.SOURCE_REMOVE;
    });
    this._timeouts.add(id);
  }
}

/**
 * On Wayland the app id arrives as the window class; the GTK application id may stay unset.
 *
 * @param {import("gi://Meta").default.Window} window
 * @returns {boolean}
 */
function isPrefsService(window) {
  return (
    window.get_wm_class() === PREFS_APP_ID || window.get_gtk_application_id() === PREFS_APP_ID
  );
}

/**
 * @param {import("gi://Meta").default.Window} window
 * @returns {import("gi://Meta").default.Window[]}
 */
function othersOf(window) {
  return global
    .get_window_actors()
    .map((actor) => actor.meta_window)
    .filter((other) => other !== window);
}

/**
 * @param {Gio.FileMonitorEvent} eventType
 * @returns {string}
 */
function eventName(eventType) {
  const name = Object.entries(Gio.FileMonitorEvent).find(([, value]) => value === eventType);
  return name === undefined ? String(eventType) : name[0];
}
