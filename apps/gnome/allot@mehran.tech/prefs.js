import Adw from "gi://Adw";
import Gio from "gi://Gio";
import GLib from "gi://GLib";
import Gtk from "gi://Gtk";

import { ExtensionPreferences } from "resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js";

import {
  DaemonConnection,
  createServiceSystem,
  glibTimers,
  readTextFile,
  writeTextFile,
} from "./daemon-io.js";
import { dataPaths } from "./lib/config.js";
import {
  METERS_HELP,
  METERS_TITLE,
  NOTIFICATIONS_TITLE,
  changedPanelLines,
  meterToggles,
  notificationToggles,
  resetPanel,
  setToggle,
  toggleEnabled,
  toggleValue,
} from "./lib/panel-toggles.js";
import { SystemdServiceLauncher } from "./lib/service-launcher.js";
import { PANEL_PAGE } from "./lib/settings-banners.js";
import { decodeSettings, encodeSettings } from "./lib/settings.js";
import {
  NOTHING_TO_RESET,
  localeNumbers,
  resetPageLines,
} from "./lib/settings-text.js";
import {
  ALLOTD_PAGES,
  SettingsWindowModel,
  TEST_COMMAND_HELP,
  TEST_COMMAND_TITLE,
  WHAT_FITS_TITLE,
  pageSections,
} from "./lib/settings-window.js";
import { matchedRules } from "./lib/rules-text.js";
import { savedRules } from "./lib/settings-editors.js";
import { DO_NOT_DISTURB_TEXT } from "./lib/snapshot.js";
import { commandTestRows, ruleListRows, sizeTableRows } from "./prefs-tables.js";
import {
  Toaster,
  bannerRows,
  confirm,
  describe,
  dimLabel,
  escaped,
  settingRows,
  textButton,
} from "./prefs-widgets.js";

/**
 * @import { PageModel } from "./lib/settings-session.js"
 * @import { BannerAction } from "./lib/settings-banners.js"
 * @import { PageState } from "./lib/settings-window.js"
 * @import { TraySettings } from "./lib/settings.js"
 * @import { Sync } from "./prefs-widgets.js"
 * @import { TableContext } from "./prefs-tables.js"
 */

// The Settings window on GNOME: the shell's preferences window with the seven pages in its view
// switcher. It runs in the org.gnome.Shell.Extensions process, which builds a new window on every
// open, so it always opens on Panel and keeps nothing between opens. fillPreferencesWindow never
// waits for allotd: the window maps only once it returns, so allotd's pages fill in after. What
// the pages show comes from the shared `SettingsWindowModel`; this file only builds libadwaita
// widgets around it. Written for GNOME Shell 48 and 49 (GTK 4.18 and 4.20, libadwaita 1.7 and
// 1.8), where every call below exists.

const DEFAULT_WIDTH = 760;
const DEFAULT_HEIGHT = 680;
const NOTIFICATION_SCHEMA = "org.gnome.desktop.notifications";
const SHOW_BANNERS = "show-banners";

export default class AllotPreferences extends ExtensionPreferences {
  /** @param {Adw.PreferencesWindow} window */
  fillPreferencesWindow(window) {
    new SettingsWindow(window);
  }
}

/**
 * One open Settings window: its session with allotd, its pages, and the panel's own settings
 * in `tray.json`.
 */
class SettingsWindow {
  #window;
  #paths = dataPaths(GLib.get_home_dir());
  #cancellable = new Gio.Cancellable();
  #toaster;
  #connection = new DaemonConnection();
  #model;
  /** @type {Map<string, AllotdPage>} */
  #pages = new Map();
  /** @type {Sync[]} */
  #panelSyncs = [];
  /** @type {TraySettings} */
  #panel;
  #closed = false;
  /**
   * Undoes what the window connected to objects that outlive it: its own signals and the
   * notification settings'.
   *
   * @type {Array<() => void>}
   */
  #disconnects = [];

  /** @param {Adw.PreferencesWindow} window */
  constructor(window) {
    this.#window = window;
    this.#toaster = new Toaster(window);
    window.set_default_size(DEFAULT_WIDTH, DEFAULT_HEIGHT);

    this.#model = new SettingsWindowModel({
      client: this.#connection.client(),
      timers: glibTimers,
      launcher: new SystemdServiceLauncher(
        {
          home: GLib.get_home_dir(),
          configHome: GLib.getenv("XDG_CONFIG_HOME"),
          root: this.#paths.root,
        },
        createServiceSystem(this.#cancellable),
      ),
      paths: this.#paths,
      reach: { kind: "bus" },
      number: localeNumbers(),
      readStopRecord: () => readTextFile(`${this.#paths.root}/stopped.json`, this.#cancellable),
      host: {
        confirm: (confirmation, onConfirm) => confirm(this.#window, confirmation, onConfirm),
        notify: (text) => this.#toaster.show(text),
        log: (text) => console.warn(text),
        openPath,
        now: () => GLib.get_monotonic_time() / 1000,
      },
    });
    this.#panel = decodeSettings(readFileNow(this.#paths.settings));

    window.add(this.#panelPage());
    for (const spec of ALLOTD_PAGES) {
      const page = new AllotdPage(spec, (action) => this.#model.act(action));
      this.#pages.set(spec.id, page);
      window.add(page.page);
    }

    this.#model.onChange = () => this.sync();
    this.#model.onAnnounce = (text) => this.#announce(text);
    this.#connection.onOwnerChanged = () => void this.#model.refresh();
    this.#connect("notify::visible-page", () => {
      if (!this.#closed) {
        this.#followPage();
      }
    });
    this.#connect("notify::is-active", () => {
      if (!this.#closed && window.is_active) {
        void this.#model.refresh();
      }
    });
    this.#connect("close-request", () => {
      this.close();
      return false;
    });
    this.#model.setVisible(true);
    this.#followPage();
    void this.#model.readSharesMemory(this.#panel.meters);
    this.sync();
  }

  /**
   * Sends what is still pending, then stops the timers. Calls are never cancelled: a change
   * sent as the window closes must still arrive.
   */
  close() {
    if (this.#closed) {
      return;
    }
    this.#closed = true;
    for (const disconnect of this.#disconnects.splice(0)) {
      disconnect();
    }
    this.#connection.onOwnerChanged = null;
    this.#connection.stopWatching();
    this.#model.onChange = null;
    this.#model.onAnnounce = null;
    this.#model.setVisible(false);
    this.#model.dispose();
  }

  /**
   * Connects a window signal that `close` disconnects.
   *
   * @param {string} signal
   * @param {() => void | boolean} handler
   */
  #connect(signal, handler) {
    const id = this.#window.connect(signal, handler);
    this.#disconnects.push(() => this.#window.disconnect(id));
  }

  /**
   * Speaks an error or an outcome without moving focus.
   *
   * @param {string} text
   */
  #announce(text) {
    if (this.#closed) {
      return;
    }
    try {
      this.#window.announce(text, Gtk.AccessibleAnnouncementPriority.MEDIUM);
    } catch (error) {
      console.warn(`Allot: cannot announce: ${error}`);
    }
  }

  /** Shows the session's state everywhere. */
  sync() {
    if (this.#closed) {
      return;
    }
    const pages = this.#model.session.pages;
    for (const page of this.#pages.values()) {
      const model = pages.find((candidate) => candidate.spec.id === page.id);
      if (model !== undefined && !page.built) {
        page.build(model, this.#pageContext(page.id));
      }
      page.sync(this.#model.pageState(page.id));
    }
    for (const part of this.#panelSyncs) {
      part();
    }
  }

  /**
   * @param {string} pageId
   * @returns {PageContext}
   */
  #pageContext(pageId) {
    const model = this.#model;
    /** @type {TableContext} */
    const table = {
      number: model.number,
      readOnly: () => model.session.readOnly,
      toaster: this.#toaster,
      changed: () => this.sync(),
      session: model.session,
      fits: model.fits,
      timers: glibTimers,
      parent: () => this.#window,
    };
    return {
      table,
      live: (groupId) => model.live(pageId, groupId),
      note: (key) => model.note(key),
      resetPage: (label) => model.resetPage(pageId, label),
      resetAll: () => model.resetAll(),
      canResetPage: () => model.changedOn(pageId).length > 0,
      canResetAll: () => model.canResetAll,
      extras: {
        service_status: (page) => this.#serviceStatus(page),
        what_fits: (page) => this.#whatFits(page),
        files: (page) => this.#files(page),
      },
    };
  }

  /**
   * What fits on this machine: one row per class from allotd's fit preview.
   *
   * @param {Adw.PreferencesPage} page
   * @returns {Sync}
   */
  #whatFits(page) {
    const group = new Adw.PreferencesGroup({ title: WHAT_FITS_TITLE });
    // Rows go in the group's list, so the caption added now stays below them.
    const caption = dimLabel();
    group.add(caption);
    page.add(group);
    /** @type {Gtk.Widget[]} */
    let rows = [];
    let signature = "";
    return () => {
      const view = this.#model.whatFits;
      const next = JSON.stringify(view);
      if (next === signature) {
        return;
      }
      signature = next;
      for (const row of rows) {
        group.remove(row);
      }
      rows = view.rows.map((fit) => {
        const row = new Adw.ActionRow({
          title: escaped(fit.label),
          subtitle: escaped(fit.text),
          subtitle_lines: 0,
        });
        group.add(row);
        return row;
      });
      caption.label = view.caption;
    };
  }

  /**
   * The Service page's status row: running, stopped by you, not answering or starting, with
   * its button.
   *
   * @param {Adw.PreferencesPage} page
   * @returns {Sync}
   */
  #serviceStatus(page) {
    const group = new Adw.PreferencesGroup({ title: "Service" });
    const row = new Adw.ActionRow({ title_lines: 0 });
    const logs = textButton("Open Logs", () => this.#model.openLogs());
    const button = textButton("", () => this.#model.pressService());
    row.add_suffix(logs);
    row.add_suffix(button);
    group.add(row);
    page.add(group);
    return () => {
      const shown = this.#model.serviceRow;
      group.visible = shown !== null;
      if (shown === null) {
        return;
      }
      row.title = escaped(shown.text);
      button.label = shown.button;
      button.sensitive = shown.canPress;
      logs.visible = shown.opensLogs;
    };
  }

  /**
   * Files: the config file, opened or shown in its folder.
   *
   * @param {Adw.PreferencesPage} page
   * @returns {Sync}
   */
  #files(page) {
    const group = new Adw.PreferencesGroup({ title: "Files" });
    const row = new Adw.ActionRow({
      title: "Config file",
      subtitle: escaped(this.#paths.config),
      subtitle_selectable: true,
    });
    row.add_suffix(textButton("Open File", () => this.#model.openFile()));
    row.add_suffix(textButton("Show in Folder", () => showInFolder(this.#paths.config)));
    group.add(row);
    page.add(group);
    return () => {};
  }

  /**
   * The Panel page: the panel's own settings in `tray.json`, which work in every allotd state
   * and save at once. The shell follows the file through its file monitor.
   *
   * @returns {Adw.PreferencesPage}
   */
  #panelPage() {
    const page = new Adw.PreferencesPage({
      name: PANEL_PAGE,
      title: "Panel",
      icon_name: "preferences-desktop-display-symbolic",
    });
    const meters = new Adw.PreferencesGroup({
      title: escaped(METERS_TITLE),
      description: escaped(METERS_HELP),
    });
    for (const toggle of meterToggles(false)) {
      const row = this.#panelSwitch(meters, toggle);
      if (toggle.key === "vram") {
        this.#panelSyncs.push(() => {
          const shown = meterToggles(this.#model.sharesMemory).find(
            (item) => item.key === toggle.key,
          );
          row.title = escaped(shown?.label ?? toggle.label);
        });
      }
      if (toggle.parent !== null) {
        row.add_prefix(new Gtk.Box({ width_request: 24 }));
      }
    }
    page.add(meters);

    const notifications = new Adw.PreferencesGroup({ title: escaped(NOTIFICATIONS_TITLE) });
    const doNotDisturb = new Adw.ActionRow({ title: escaped(DO_NOT_DISTURB_TEXT), title_lines: 0 });
    doNotDisturb.add_prefix(new Gtk.Image({ icon_name: "notifications-disabled-symbolic" }));
    notifications.add(doNotDisturb);
    const notificationSettings = new Gio.Settings({ schema_id: NOTIFICATION_SCHEMA });
    const showDoNotDisturb = () => {
      doNotDisturb.visible = !notificationSettings.get_boolean(SHOW_BANNERS);
    };
    const signal = notificationSettings.connect(`changed::${SHOW_BANNERS}`, showDoNotDisturb);
    this.#disconnects.push(() => notificationSettings.disconnect(signal));
    showDoNotDisturb();
    for (const toggle of notificationToggles()) {
      this.#panelSwitch(notifications, toggle);
    }
    page.add(notifications);

    const reset = new Adw.PreferencesGroup();
    const resetRow = new Adw.ButtonRow({ title: "Reset Page…" });
    resetRow.connect("activated", () => {
      const lines = changedPanelLines(this.#panel, this.#model.sharesMemory);
      if (lines.length > 0) {
        confirm(this.#window, resetPageLines("Panel", lines), () =>
          this.#updatePanel((settings) => resetPanel(settings)));
      }
    });
    reset.add(resetRow);
    page.add(reset);
    this.#panelSyncs.push(() => {
      const changed = changedPanelLines(this.#panel, this.#model.sharesMemory).length > 0;
      resetRow.sensitive = changed;
      resetRow.tooltip_text = changed ? null : NOTHING_TO_RESET;
    });
    return page;
  }

  /**
   * @param {Adw.PreferencesGroup} group
   * @param {import("./lib/panel-toggles.js").PanelToggle} toggle
   * @returns {Adw.SwitchRow}
   */
  #panelSwitch(group, toggle) {
    const row = new Adw.SwitchRow({
      title: escaped(toggle.label),
      subtitle: escaped(toggle.help),
    });
    let syncing = false;
    row.connect("notify::active", () => {
      if (!syncing) {
        this.#updatePanel((settings) => setToggle(settings, toggle, row.active));
      }
    });
    describe(row, toggle.help);
    group.add(row);
    this.#panelSyncs.push(() => {
      syncing = true;
      row.active = toggleValue(this.#panel, toggle);
      syncing = false;
      row.sensitive = toggleEnabled(this.#panel, toggle);
    });
    return row;
  }

  /**
   * Changes `tray.json`: read again first, so what the shell wrote meanwhile (the Resources
   * card's state, the first-run flags) is kept, then written atomically.
   *
   * @param {(settings: TraySettings) => void} change
   */
  #updatePanel(change) {
    const settings = decodeSettings(readFileNow(this.#paths.settings));
    change(settings);
    this.#panel = settings;
    writeTextFile(this.#paths.settings, encodeSettings(settings)).catch((error) => {
      console.warn(`Allot: cannot save ${this.#paths.settings}: ${error.message}`);
      this.#toaster.show(`Could not save the panel settings: ${error.message}`);
    });
    this.sync();
  }

  #followPage() {
    this.#model.showPage(this.#window.visible_page_name ?? "");
  }
}

/**
 * @typedef {object} PageContext
 * @property {TableContext} table
 * @property {(groupId: string) => string | null} live
 * @property {(key: string) => string | null} note
 * @property {(label: string) => void} resetPage
 * @property {() => void} resetAll
 * @property {() => boolean} canResetPage
 * @property {() => boolean} canResetAll
 * @property {Record<"service_status" | "what_fits" | "files", (page: Adw.PreferencesPage) => Sync>}
 *   extras The groups a page adds to the spec's.
 */

/**
 * One of allotd's pages: banners and the save status first, built from the spec once
 * allotd's settings have arrived, then Reset.
 */
class AllotdPage {
  page;
  id;
  built = false;
  #banners;
  #bannerGroup;
  /** @type {Adw.PreferencesGroup[]} */
  #settingGroups = [];
  /** @type {Sync[]} */
  #syncs = [];
  /** @type {(readOnly: boolean) => void} */
  #syncReset = () => {};

  /**
   * @param {{ id: string, label: string, gnomeIcon: string }} spec
   * @param {(action: BannerAction) => void} onAction
   */
  constructor(spec, onAction) {
    this.id = spec.id;
    this.page = new Adw.PreferencesPage({
      name: spec.id,
      title: spec.label,
      icon_name: spec.gnomeIcon,
    });
    this.#bannerGroup = new Adw.PreferencesGroup();
    this.#banners = bannerRows(this.#bannerGroup, onAction);
    this.page.add(this.#bannerGroup);
  }

  /**
   * Adds the page's groups from the spec, once.
   *
   * @param {PageModel} model
   * @param {PageContext} context
   */
  build(model, context) {
    this.built = true;
    if (model.spec.intro !== null) {
      this.page.description = escaped(model.spec.intro);
    }
    for (const section of pageSections(model)) {
      switch (section.kind) {
        case "extra":
          this.#syncs.push(context.extras[section.id](this.page));
          break;
        case "settings":
          this.#addGroup(context, section.title, section.settings, section.live);
          break;
        case "sizes": {
          const group = new Adw.PreferencesGroup({ title: escaped(section.title) });
          this.#settingGroups.push(group);
          this.page.add(group);
          this.#syncs.push(sizeTableRows(context.table, group, section.table));
          break;
        }
        case "rules":
          this.#addRules(context, section.title, section.rules);
          break;
        case "reset":
          this.#addReset(context, model.spec.label, section.all);
          break;
      }
    }
  }

  /** @param {PageState} state */
  sync(state) {
    this.#bannerGroup.description = escaped(state.status);
    this.#banners(state.banners);
    this.#bannerGroup.visible = state.banners.length > 0 || state.status !== "";
    for (const group of this.#settingGroups) {
      group.sensitive = !state.readOnly;
    }
    for (const part of this.#syncs) {
      part();
    }
    this.#syncReset(state.readOnly);
  }

  /**
   * @param {PageContext} context
   * @param {string} title
   * @param {import("./lib/setting-values.js").ValueModel<unknown>[]} settings
   * @param {string | null} liveGroupId The group whose live sentence ends this one.
   */
  #addGroup(context, title, settings, liveGroupId) {
    const group = new Adw.PreferencesGroup({ title: escaped(title) });
    this.#settingGroups.push(group);
    this.page.add(group);
    for (const setting of settings) {
      this.#syncs.push(settingRows(context.table, group, setting, context.note));
    }
    const sentence = liveGroupId === null ? null : context.live(liveGroupId);
    if (sentence !== null && liveGroupId !== null) {
      const live = dimLabel();
      group.add(live);
      this.#syncs.push(() => {
        live.label = context.live(liveGroupId) ?? "";
      });
    }
  }

  /**
   * Test a command above the rules, its matches marked on them.
   *
   * @param {PageContext} context
   * @param {string} title
   * @param {import("./lib/setting-values.js").RuleListModel} rules
   */
  #addRules(context, title, rules) {
    const test = new Adw.PreferencesGroup({
      title: escaped(TEST_COMMAND_TITLE),
      description: escaped(TEST_COMMAND_HELP),
    });
    const rulesGroup = new Adw.PreferencesGroup({ title: escaped(title) });
    this.page.add(test);
    this.page.add(rulesGroup);
    this.#settingGroups.push(test, rulesGroup);
    const list = ruleListRows(context.table, rulesGroup, rules);
    commandTestRows(
      context.table,
      test,
      TEST_COMMAND_HELP,
      () => savedRules(rules),
      (result) => list.setMatched(matchedRules(result)),
    );
    this.#syncs.push(list.sync);
  }

  /**
   * Reset Page…, and on Service Reset All Service Settings….
   *
   * @param {PageContext} context
   * @param {string} label
   * @param {boolean} withAll
   */
  #addReset(context, label, withAll) {
    const group = new Adw.PreferencesGroup();
    const page = new Adw.ButtonRow({ title: "Reset Page…" });
    page.connect("activated", () => context.resetPage(label));
    group.add(page);
    /** @type {Adw.ButtonRow | null} */
    let all = null;
    if (withAll) {
      all = new Adw.ButtonRow({ title: "Reset All Service Settings…" });
      all.connect("activated", () => context.resetAll());
      group.add(all);
    }
    this.page.add(group);
    this.#syncReset = (readOnly) => {
      const changed = context.canResetPage();
      page.sensitive = changed && !readOnly;
      page.tooltip_text = changed ? null : NOTHING_TO_RESET;
      if (all !== null) {
        const any = context.canResetAll();
        all.sensitive = any && !readOnly;
        all.tooltip_text = any ? null : NOTHING_TO_RESET;
      }
    };
  }
}

/**
 * A small local file read at once; the preferences process is not the compositor, so this
 * blocks nothing the user sees.
 *
 * @param {string} path
 * @returns {string | null}
 */
function readFileNow(path) {
  try {
    const [, contents] = GLib.file_get_contents(path);
    return new TextDecoder().decode(contents);
  } catch {
    return null;
  }
}

/** @param {string} path */
function openPath(path) {
  const uri = GLib.filename_to_uri(path, null);
  try {
    Gio.AppInfo.launch_default_for_uri(uri, null);
  } catch (error) {
    console.warn(`Allot: cannot open ${uri}: ${error}`);
  }
}

/**
 * Shows the file selected in the file manager, or opens its folder when no file manager offers
 * that.
 *
 * @param {string} path
 */
function showInFolder(path) {
  const uri = GLib.filename_to_uri(path, null);
  Gio.DBus.session.call(
    "org.freedesktop.FileManager1",
    "/org/freedesktop/FileManager1",
    "org.freedesktop.FileManager1",
    "ShowItems",
    new GLib.Variant("(ass)", [[uri], ""]),
    null,
    Gio.DBusCallFlags.NONE,
    -1,
    null,
    (connection, result) => {
      try {
        connection?.call_finish(result);
      } catch {
        openPath(GLib.path_get_dirname(path));
      }
    },
  );
}
