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
import { CancelledError } from "./lib/client.js";
import { DEFAULT_PORT, dataPaths, portFromToml } from "./lib/config.js";
import { FitPreviews, NOT_YET_TEXT } from "./lib/fit-previews.js";
import {
  benchmarksText,
  estimatesText,
  fitsCaption,
  fitsText,
  headroomText,
  historyText,
} from "./lib/live-text.js";
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
import { resourcesView } from "./lib/resources-view.js";
import { START_TIMEOUT_MS } from "./lib/poller.js";
import { SystemdServiceLauncher, restartService } from "./lib/service-launcher.js";
import { STOP_CONFIRM_TITLE, stopConfirmation } from "./lib/service-menu.js";
import { serviceButtonText, serviceRow, sessionServiceStatus } from "./lib/service-row.js";
import {
  NumberModel,
  RuleListModel,
  SizeTableModel,
  SwitchModel,
} from "./lib/setting-values.js";
import { PANEL_PAGE, SERVICE_PAGE, restartNote, settingsBanners } from "./lib/settings-banners.js";
import { SettingsSession } from "./lib/settings-session.js";
import { decodeSettings, encodeSettings } from "./lib/settings.js";
import {
  LOADING_TEXT,
  NOTHING_TO_RESET,
  localeNumbers,
  resetAllConfirmation,
  resetPageConfirmation,
  resetPageLines,
  saveStatusText,
} from "./lib/settings-text.js";
import { matchedRules } from "./lib/rules-text.js";
import { DO_NOT_DISTURB_TEXT } from "./lib/snapshot.js";
import { commandTestRows, ruleListRows, sizeTableRows } from "./prefs-tables.js";
import {
  Toaster,
  bannerRows,
  confirm,
  dimLabel,
  escaped,
  settingRows,
  textButton,
} from "./prefs-widgets.js";

/**
 * @import { PageModel } from "./lib/settings-session.js"
 * @import { BannerAction } from "./lib/settings-banners.js"
 * @import { TraySettings } from "./lib/settings.js"
 * @import { Sync } from "./prefs-widgets.js"
 * @import { TableContext } from "./prefs-tables.js"
 */

// The Settings window on GNOME (DESIGN.md section 10): the shell's preferences window with the
// seven pages in its view switcher. It runs in the org.gnome.Shell.Extensions process, which
// builds a new window on every open, so it always opens on Panel (D14) and keeps nothing between
// opens. fillPreferencesWindow never waits for allotd: the window maps only once it returns
// (M0.3), so allotd's pages fill in after.

const DEFAULT_WIDTH = 760;
const DEFAULT_HEIGHT = 680;
const NOTIFICATION_SCHEMA = "org.gnome.desktop.notifications";
const SHOW_BANNERS = "show-banners";
const START_POLL_MS = 500;
const PORT_KEY = "daemon.port";
const PAGES_WITH_FITS = ["admission", "sizes"];

/** allotd's pages in the order DESIGN.md gives, with their switcher icons. */
const ALLOTD_PAGES = [
  { id: "admission", label: "Admission", icon: "preferences-system-symbolic" },
  { id: "sizes", label: "Sizes", icon: "view-grid-symbolic" },
  { id: "commands", label: "Commands", icon: "utilities-terminal-symbolic" },
  { id: "benchmarks", label: "Benchmarks", icon: "alarm-symbolic" },
  { id: "leases", label: "Leases", icon: "document-open-recent-symbolic" },
  { id: "service", label: "Service", icon: "network-server-symbolic" },
];

export default class AllotPreferences extends ExtensionPreferences {
  /** @param {Adw.PreferencesWindow} window */
  fillPreferencesWindow(window) {
    const settings = new SettingsWindow(window);
    window.connect("close-request", () => {
      settings.close();
      return false;
    });
  }
}

/**
 * One open Settings window: its session with allotd, its pages, and the panel's own settings
 * in `tray.json`.
 */
class SettingsWindow {
  #window;
  #paths = dataPaths(GLib.get_home_dir());
  #number = localeNumbers();
  #cancellable = new Gio.Cancellable();
  #toaster;
  #client;
  #session;
  #fits;
  #launcher;
  /** @type {Map<string, AllotdPage>} */
  #pages = new Map();
  /** @type {Sync[]} */
  #panelSyncs = [];
  /** @type {TraySettings} */
  #panel;
  #sharesMemory = false;
  /** @type {string | null} */
  #version = null;
  #starting = false;
  /** @type {number} */
  #startedAt = 0;
  #closed = false;

  /** @param {Adw.PreferencesWindow} window */
  constructor(window) {
    this.#window = window;
    this.#toaster = new Toaster(window);
    window.set_default_size(DEFAULT_WIDTH, DEFAULT_HEIGHT);

    const toml = readFileNow(this.#paths.config);
    // Never cancelled: a change sent as the window closes must still arrive (M0.3).
    this.#client = new DaemonConnection().client({
      port: toml === null ? DEFAULT_PORT : portFromToml(toml),
      readToken: () => readTextFile(this.#paths.token, this.#cancellable),
    });
    this.#session = new SettingsSession(this.#client, glibTimers, {
      platform: "linux",
      readStopRecord: () => readTextFile(`${this.#paths.root}/stopped.json`, this.#cancellable),
    });
    this.#fits = new FitPreviews(this.#session, glibTimers);
    this.#launcher = new SystemdServiceLauncher(
      {
        home: GLib.get_home_dir(),
        configHome: GLib.getenv("XDG_CONFIG_HOME"),
        root: this.#paths.root,
      },
      createServiceSystem(this.#cancellable),
    );
    this.#panel = decodeSettings(readFileNow(this.#paths.settings));

    window.add(this.#panelPage());
    for (const spec of ALLOTD_PAGES) {
      const page = new AllotdPage(spec, (action) => this.#act(action));
      this.#pages.set(spec.id, page);
      window.add(page.page);
    }

    let previousStatus = this.#session.status;
    this.#session.onChange = () => {
      if (this.#session.status !== previousStatus) {
        previousStatus = this.#session.status;
        this.#statusChanged();
      }
      this.#fits.sessionChanged();
      this.sync();
    };
    this.#fits.onChange = () => this.sync();
    window.connect("notify::visible-page", () => this.#followPage());
    window.connect("notify::is-active", () => {
      if (window.is_active) {
        void this.#refresh();
      }
    });
    this.#session.setVisible(true);
    this.#followPage();
    void this.#readSharesMemory();
    this.sync();
  }

  /** Sends what is still pending, then stops the timers; the request outlives the window. */
  close() {
    this.#closed = true;
    this.#session.onChange = null;
    this.#fits.onChange = null;
    this.#session.setVisible(false);
    this.#fits.dispose();
    this.#session.dispose();
  }

  /** Shows the session's state everywhere. */
  sync() {
    if (this.#closed) {
      return;
    }
    const pages = this.#session.pages;
    for (const page of this.#pages.values()) {
      const model = pages.find((candidate) => candidate.spec.id === page.id);
      if (model !== undefined && !page.built) {
        page.build(model, this.#pageContext(page.id));
      }
      page.sync(this.#pageState(page.id));
    }
    for (const part of this.#panelSyncs) {
      part();
    }
  }

  /**
   * What an allotd page shows above and around its settings.
   *
   * @param {string} pageId
   * @returns {PageState}
   */
  #pageState(pageId) {
    const session = this.#session;
    const save = session.saveStatus;
    // While allotd does not answer, "All changes saved" would say more than is known.
    const quiet = session.status !== "ready" && save.kind === "saved";
    const status = session.status === "loading" ? LOADING_TEXT : quiet ? "" : saveStatusText(save);
    return {
      banners: settingsBanners(session, pageId, this.#client.port, this.#number),
      status,
      readOnly: session.readOnly,
    };
  }

  /**
   * @param {string} pageId
   * @returns {PageContext}
   */
  #pageContext(pageId) {
    /** @type {TableContext} */
    const table = {
      number: this.#number,
      readOnly: () => this.#session.readOnly,
      toaster: this.#toaster,
      changed: () => this.sync(),
      session: this.#session,
      fits: this.#fits,
      timers: glibTimers,
      parent: () => this.#window,
    };
    return {
      table,
      live: (groupId) => this.#live(pageId, groupId),
      note: (key) => restartNote(this.#session, key),
      resetPage: (label) => this.#resetPage(pageId, label),
      resetAll: () => this.#resetAll(),
      changedOn: () => (this.#session.readOnly ? [] : this.#session.changedOn(pageId)),
      extras: this.#extras(pageId),
    };
  }

  /**
   * A group's live sentence (DESIGN.md D10), or `null` for a group without one.
   *
   * @param {string} pageId
   * @param {string} groupId
   * @returns {string | null}
   */
  #live(pageId, groupId) {
    const number = this.#number;
    /** @param {string} key */
    const value = (key) => {
      const model = this.#session.setting(key);
      return model instanceof NumberModel ? (model.value ?? 0) : 0;
    };
    switch (`${pageId}/${groupId}`) {
      case "admission/keep_free":
        if (this.#fits.unavailable !== null) {
          return this.#fits.unavailable;
        }
        return this.#fits.latest === null
          ? NOT_YET_TEXT
          : headroomText(this.#fits.latest.kept_free, number);
      case "admission/learn": {
        const lower = this.#session.setting("estimates.lower");
        return estimatesText(
          {
            lower: lower instanceof SwitchModel ? lower.value : true,
            minRuns: value("estimates.min_runs"),
            marginPct: value("estimates.margin_pct"),
          },
          number,
        );
      }
      case "benchmarks/quiet":
        return benchmarksText(
          {
            cpuPct: value("benchmark.quiet_cpu_pct"),
            gpuPct: value("benchmark.quiet_gpu_pct"),
            quietSecs: value("benchmark.quiet_secs"),
            maxWaitSecs: value("benchmark.max_quiet_wait_secs"),
          },
          number,
        );
      case "leases/history":
        return historyText(
          {
            leaseDays: value("retention.lease_days"),
            historyDays: value("retention.history_days"),
            keepRuns: value("retention.keep_runs_per_job"),
            maxRuns: value("retention.max_job_runs"),
          },
          number,
        );
      default:
        return null;
    }
  }

  /**
   * The groups a page adds to the spec's, by where they go.
   *
   * @param {string} pageId
   * @returns {PageExtras}
   */
  #extras(pageId) {
    switch (pageId) {
      case "admission":
        return { after: { learn: (page) => this.#whatFits(page) } };
      case SERVICE_PAGE:
        return {
          top: (page) => this.#serviceStatus(page),
          end: (page) => this.#files(page),
          resetAll: true,
        };
      default:
        return {};
    }
  }

  /**
   * What fits on this machine: one row per class from allotd's fit preview.
   *
   * @param {Adw.PreferencesPage} page
   * @returns {Sync}
   */
  #whatFits(page) {
    const group = new Adw.PreferencesGroup({ title: "What Fits on This Machine" });
    // Rows go in the group's list, so the caption added now stays below them.
    const caption = dimLabel();
    group.add(caption);
    page.add(group);
    /** @type {Gtk.Widget[]} */
    let rows = [];
    let signature = "";
    return () => {
      const preview = this.#fits.unavailable === null ? this.#fits.latest : null;
      const next = JSON.stringify([preview, this.#fits.unavailable]);
      if (next === signature) {
        return;
      }
      signature = next;
      for (const row of rows) {
        group.remove(row);
      }
      const classes = this.#session.setting("classes");
      const labels = classes instanceof SizeTableModel ? classes.rows : [];
      rows = (preview?.classes ?? []).map((fit) => {
        const label = labels.find((view) => view.row.name === fit.name)?.label ?? fit.name;
        const now = fitsText(fit.now, fit.min_cores, this.#number);
        const idle = fitsText(fit.idle, fit.min_cores, this.#number);
        const row = new Adw.ActionRow({
          title: escaped(label),
          subtitle: escaped(`Now: ${now} · When idle: ${idle}`),
          subtitle_lines: 0,
        });
        group.add(row);
        return row;
      });
      caption.label =
        this.#fits.unavailable ??
        (preview === null ? NOT_YET_TEXT : fitsCaption(preview.cpu_busy_pct, this.#number));
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
    const logs = textButton("Open Logs", () => this.#openLogs());
    const button = textButton("", () => this.#pressService());
    row.add_suffix(logs);
    row.add_suffix(button);
    group.add(row);
    page.add(group);
    return () => {
      const status = sessionServiceStatus(this.#session.status, this.#starting);
      group.visible = status !== null;
      if (status === null) {
        return;
      }
      const shown = serviceRow(status, this.#session.status, this.#version, this.#client.port);
      row.title = escaped(shown.text);
      button.label = serviceButtonText(shown.button);
      button.sensitive = shown.button !== "starting";
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
    row.add_suffix(textButton("Open File", () => openPath(this.#paths.config)));
    row.add_suffix(textButton("Show in Folder", () => showInFolder(this.#paths.config)));
    group.add(row);
    page.add(group);
    return () => {};
  }

  /**
   * The Panel page: the panel's own settings in `tray.json`, which work in every allotd state
   * and save at once. The shell follows the file through its file monitor (M0.3).
   *
   * @returns {Adw.PreferencesPage}
   */
  #panelPage() {
    const page = new Adw.PreferencesPage({
      name: PANEL_PAGE,
      title: "Panel",
      icon_name: "preferences-desktop-display-symbolic",
    });
    const meters = new Adw.PreferencesGroup({ title: METERS_TITLE, description: METERS_HELP });
    for (const toggle of meterToggles(false)) {
      const row = this.#panelSwitch(meters, toggle);
      if (toggle.key === "vram") {
        this.#panelSyncs.push(() => {
          row.title = escaped(meterToggles(this.#sharesMemory)[3]?.label ?? toggle.label);
        });
      }
      if (toggle.parent !== null) {
        row.add_prefix(new Gtk.Box({ width_request: 24 }));
      }
    }
    page.add(meters);

    const notifications = new Adw.PreferencesGroup({ title: NOTIFICATIONS_TITLE });
    const doNotDisturb = new Adw.ActionRow({ title: escaped(DO_NOT_DISTURB_TEXT), title_lines: 0 });
    doNotDisturb.add_prefix(new Gtk.Image({ icon_name: "notifications-disabled-symbolic" }));
    notifications.add(doNotDisturb);
    const notificationSettings = new Gio.Settings({ schema_id: NOTIFICATION_SCHEMA });
    const showDoNotDisturb = () => {
      doNotDisturb.visible = !notificationSettings.get_boolean(SHOW_BANNERS);
    };
    const signal = notificationSettings.connect(`changed::${SHOW_BANNERS}`, showDoNotDisturb);
    this.#window.connect("close-request", () => {
      notificationSettings.disconnect(signal);
      return false;
    });
    showDoNotDisturb();
    for (const toggle of notificationToggles()) {
      this.#panelSwitch(notifications, toggle);
    }
    page.add(notifications);

    const reset = new Adw.PreferencesGroup();
    const resetRow = new Adw.ButtonRow({ title: "Reset Page…" });
    resetRow.connect("activated", () => {
      const lines = changedPanelLines(this.#panel, this.#sharesMemory);
      if (lines.length > 0) {
        confirm(this.#window, resetPageLines("Panel", lines), () =>
          this.#updatePanel((settings) => resetPanel(settings)));
      }
    });
    reset.add(resetRow);
    page.add(reset);
    this.#panelSyncs.push(() => {
      const changed = changedPanelLines(this.#panel, this.#sharesMemory).length > 0;
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

  /** Whether a GPU shares system memory, so the VRAM meter reads "GPU memory". */
  async #readSharesMemory() {
    try {
      const result = await this.#client.poll();
      if (result.state !== null) {
        this.#sharesMemory = resourcesView("running", result.state, this.#panel.meters, false)
          .sharesMemory;
        this.sync();
      }
    } catch (error) {
      if (!(error instanceof CancelledError)) {
        throw error;
      }
    }
  }

  /** Fit previews run only while a page that shows them is open. */
  #followPage() {
    const name = this.#window.visible_page_name ?? "";
    this.#fits.setActive(PAGES_WITH_FITS.includes(name));
  }

  async #refresh() {
    try {
      await this.#session.refresh();
    } catch (error) {
      if (!(error instanceof CancelledError)) {
        throw error;
      }
    }
  }

  /** The session started or stopped answering: a start under way ends, the version is read. */
  #statusChanged() {
    if (this.#session.status === "ready") {
      this.#starting = false;
      this.#client
        .healthVersion()
        .then((version) => {
          this.#version = version;
          this.sync();
        })
        .catch(() => {});
    }
  }

  /** @param {BannerAction} action */
  #act(action) {
    switch (action) {
      case "start_service":
        void this.#startService();
        break;
      case "restart_service":
        void this.#restartService();
        break;
      case "try_now":
      case "check_again":
        void this.#refresh();
        break;
      case "open_logs":
        this.#openLogs();
        break;
      case "open_file":
        openPath(this.#paths.config);
        break;
      case "apply_mine":
        this.#session.applyMine();
        break;
      case "keep_file_values":
        this.#session.keepFileValues();
        break;
      case "save_again":
        void this.#session.flush();
        break;
    }
  }

  #pressService() {
    const status = sessionServiceStatus(this.#session.status, this.#starting);
    if (status === "running" || status === "incompatible") {
      void this.#stopService();
    } else if (status === "stopped") {
      void this.#startService();
    }
  }

  /** Stop asks only while leases are active; the stop lasts until a start (M2.1). */
  async #stopService() {
    const poll = await this.#client.poll().catch(() => null);
    const question = stopConfirmation(poll?.state ?? null);
    const stop = async () => {
      const problem = await this.#client.shutdown();
      if (problem !== null) {
        this.#toaster.show(`The service did not stop: ${problem}`);
      }
      await this.#refresh();
    };
    if (question === null) {
      await stop();
      return;
    }
    confirm(
      this.#window,
      { title: STOP_CONFIRM_TITLE, body: question, button: "Stop Service" },
      () => void stop(),
    );
  }

  async #startService() {
    this.#beginStarting();
    const problem = await this.#launcher.start();
    if (problem !== null) {
      this.#starting = false;
      this.#toaster.show(`The service did not start: ${problem}`);
    }
    this.sync();
  }

  /** For a new port or a refused key: through systemd when its unit runs allotd. */
  async #restartService() {
    this.#beginStarting();
    const saved = this.#session.setting(PORT_KEY)?.saved;
    const port = typeof saved === "number" ? saved : this.#client.port;
    const problem = await restartService(this.#client, this.#launcher, glibTimers, port);
    if (problem !== null) {
      this.#starting = false;
      this.#toaster.show(`The service did not restart: ${problem}`);
    }
    this.sync();
  }

  /** Shows Starting… and asks allotd often until it answers or the start times out. */
  #beginStarting() {
    this.#starting = true;
    this.#startedAt = GLib.get_monotonic_time() / 1000;
    this.sync();
    const tick = () => {
      if (this.#closed || !this.#starting) {
        return;
      }
      if (GLib.get_monotonic_time() / 1000 - this.#startedAt > START_TIMEOUT_MS) {
        this.#starting = false;
        this.sync();
        return;
      }
      void this.#refresh().then(() => glibTimers.setTimeout(tick, START_POLL_MS));
    };
    glibTimers.setTimeout(tick, START_POLL_MS);
  }

  #openLogs() {
    openPath(this.#paths.logs);
  }

  /**
   * @param {string} pageId
   * @param {string} label
   */
  #resetPage(pageId, label) {
    const changed = this.#session.changedOn(pageId);
    if (changed.length > 0) {
      confirm(this.#window, resetPageConfirmation(label, changed, this.#number), () =>
        this.#session.resetChanged(pageId));
    }
  }

  #resetAll() {
    const changed = this.#session.changedSettings();
    if (changed.length > 0) {
      confirm(this.#window, resetAllConfirmation(changed.length), () =>
        this.#session.resetChanged(null));
    }
  }
}

/**
 * @typedef {object} PageState
 * @property {import("./lib/settings-banners.js").SettingsBanner[]} banners
 * @property {string} status The save status, or Loading settings….
 * @property {boolean} readOnly
 */

/**
 * @typedef {object} PageExtras
 * @property {(page: Adw.PreferencesPage) => Sync} [top] Groups above the spec's.
 * @property {Record<string, (page: Adw.PreferencesPage) => Sync>} [after] Groups after a
 *   spec group, by its id.
 * @property {(page: Adw.PreferencesPage) => Sync} [end] Groups after the spec's.
 * @property {boolean} [resetAll] Whether Reset All Service Settings… sits beside Reset Page….
 */

/**
 * @typedef {object} PageContext
 * @property {TableContext} table
 * @property {(groupId: string) => string | null} live
 * @property {(key: string) => string | null} note
 * @property {(label: string) => void} resetPage
 * @property {() => void} resetAll
 * @property {() => import("./lib/setting-values.js").ValueModel<unknown>[]} changedOn
 * @property {PageExtras} extras
 */

/**
 * One of allotd's pages: banners and the save status first, built from the spec once
 * `GET /settings` has answered, then Reset (DESIGN.md sections 4 to 7).
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
   * @param {{ id: string, label: string, icon: string }} spec
   * @param {(action: BannerAction) => void} onAction
   */
  constructor(spec, onAction) {
    this.id = spec.id;
    this.page = new Adw.PreferencesPage({
      name: spec.id,
      title: spec.label,
      icon_name: spec.icon,
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
    const page = this.page;
    const extras = context.extras;
    if (model.spec.intro !== null) {
      page.description = model.spec.intro;
    }
    if (extras.top !== undefined) {
      this.#syncs.push(extras.top(page));
    }
    /**
     * @type {Array<{
     *   label: string,
     *   groupId: string,
     *   settings: PageModel["groups"][number]["settings"],
     * }>}
     */
    const advanced = [];
    for (const group of model.groups) {
      const normal = group.settings.filter((setting) => !setting.spec.advanced);
      const rare = group.settings.filter((setting) => setting.spec.advanced);
      if (normal.length > 0) {
        const live = rare.length === 0 ? group.spec.id : null;
        this.#addGroup(context, group.spec.label, normal, live);
      }
      if (rare.length > 0) {
        advanced.push({ label: group.spec.label, groupId: group.spec.id, settings: rare });
      }
      const after = extras.after?.[group.spec.id];
      if (after !== undefined) {
        this.#syncs.push(after(page));
      }
    }
    if (extras.end !== undefined) {
      this.#syncs.push(extras.end(page));
    }
    for (const group of advanced) {
      // The live sentence goes with the group's last part.
      this.#addGroup(context, `Advanced: ${group.label}`, group.settings, group.groupId);
    }
    this.#addReset(context, model.spec.label, extras.resetAll === true);
  }

  /** @param {PageState} state */
  sync(state) {
    this.#bannerGroup.description = state.status;
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
    const group = new Adw.PreferencesGroup({ title });
    this.#settingGroups.push(group);
    const first = settings[0];
    if (first instanceof RuleListModel) {
      this.#addRules(context, group, first);
      return;
    }
    this.page.add(group);
    if (first instanceof SizeTableModel) {
      this.#syncs.push(sizeTableRows(context.table, group, first));
      return;
    }
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
   * Test a command above the rules, its matches marked on them (D6).
   *
   * @param {PageContext} context
   * @param {Adw.PreferencesGroup} rulesGroup
   * @param {RuleListModel} rules
   */
  #addRules(context, rulesGroup, rules) {
    const test = new Adw.PreferencesGroup({
      title: "Test a Command",
      description: "Shows how the hook would treat this command with the rules below.",
    });
    this.page.add(test);
    this.page.add(rulesGroup);
    this.#settingGroups.push(test);
    const list = ruleListRows(context.table, rulesGroup, rules);
    commandTestRows(
      context.table,
      test,
      () => ({ changes: [], rules: rules.value }),
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
      const changed = context.changedOn().length > 0;
      page.sensitive = changed && !readOnly;
      page.tooltip_text = changed ? null : NOTHING_TO_RESET;
      if (all !== null) {
        const any = context.table.session.changedSettings().length > 0;
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

