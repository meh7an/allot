import { CancelledError } from "./client.js";
import { FitPreviews, NOT_YET_TEXT } from "./fit-previews.js";
import {
  benchmarksText,
  estimatesText,
  fitsCaption,
  fitsText,
  headroomText,
  historyText,
} from "./live-text.js";
import { START_TIMEOUT_MS } from "./poller.js";
import { resourcesView } from "./resources-view.js";
import { restartService } from "./service-launcher.js";
import { STOP_CONFIRM_TITLE, stopConfirmation } from "./service-menu.js";
import { serviceButtonText, serviceRow, sessionServiceStatus } from "./service-row.js";
import {
  NumberModel,
  RuleListModel,
  SizeTableModel,
  SwitchModel,
} from "./setting-values.js";
import { SERVICE_PAGE, restartNote, settingsBanners } from "./settings-banners.js";
import { SettingsSession } from "./settings-session.js";
import {
  LOADING_TEXT,
  resetAllConfirmation,
  resetPageConfirmation,
  saveStatusText,
} from "./settings-text.js";

/**
 * @import { PollResult } from "./client.js"
 * @import { DataPaths } from "./config.js"
 * @import { Timers } from "./poller.js"
 * @import { MeterToggles } from "./settings.js"
 * @import { RestartClient, RestartLauncher } from "./service-launcher.js"
 * @import { BannerAction, SettingsBanner } from "./settings-banners.js"
 * @import { PageModel, SettingsClient } from "./settings-session.js"
 * @import { Confirmation } from "./settings-text.js"
 * @import { ValueModel } from "./setting-values.js"
 */

// The Settings window of the GNOME and KDE panels without its toolkit (DESIGN.md sections 2 and
// 5 to 8): the session with allotd, what each page shows, the live sentences, and the service's
// Start, Stop and Restart. Each panel builds its widgets from this and calls back into it.

/** allotd's pages in the order DESIGN.md gives, with each panel's icon for them. */
export const ALLOTD_PAGES = [
  {
    id: "admission",
    label: "Admission",
    gnomeIcon: "preferences-system-symbolic",
    kdeIcon: "view-statistics",
  },
  { id: "sizes", label: "Sizes", gnomeIcon: "view-grid-symbolic", kdeIcon: "view-list-details" },
  {
    id: "commands",
    label: "Commands",
    gnomeIcon: "utilities-terminal-symbolic",
    kdeIcon: "utilities-terminal",
  },
  { id: "benchmarks", label: "Benchmarks", gnomeIcon: "alarm-symbolic", kdeIcon: "chronometer" },
  {
    id: "leases",
    label: "Leases",
    gnomeIcon: "document-open-recent-symbolic",
    kdeIcon: "password-copy",
  },
  {
    id: "service",
    label: "Service",
    gnomeIcon: "network-server-symbolic",
    kdeIcon: "network-server",
  },
];

/** The pages whose sentences use allotd's fit preview, which runs only while one is shown. */
export const PAGES_WITH_FITS = ["admission", "sizes"];

export const WHAT_FITS_TITLE = "What Fits on This Machine";

export const TEST_COMMAND_TITLE = "Test a Command";

export const TEST_COMMAND_HELP =
  "Shows how the hook would treat this command with the rules below.";

export const TEST_RULE_HELP = "Shows how the hook would treat this command with this rule.";

const PORT_KEY = "daemon.port";
const START_POLL_MS = 500;

/**
 * A part of an allotd page, in the order it shows.
 *
 * @typedef {(
 *   | { kind: "extra", id: "service_status" | "what_fits" | "files" }
 *   | { kind: "settings", title: string, settings: ValueModel<unknown>[], live: string | null }
 *   | { kind: "sizes", title: string, table: SizeTableModel }
 *   | { kind: "rules", title: string, rules: RuleListModel }
 *   | { kind: "reset", all: boolean }
 * )} PageSection
 */

/**
 * The parts of an allotd page: the spec's groups, rarely changed keys last under an Advanced
 * heading (DESIGN.md D11), the page's own groups where they go, then Reset.
 *
 * @param {PageModel} model
 * @returns {PageSection[]}
 */
export function pageSections(model) {
  const pageId = model.spec.id;
  /** @type {PageSection[]} */
  const sections = [];
  if (pageId === SERVICE_PAGE) {
    sections.push({ kind: "extra", id: "service_status" });
  }
  /** @type {PageSection[]} */
  const advanced = [];
  for (const group of model.groups) {
    const normal = group.settings.filter((setting) => !setting.spec.advanced);
    const rare = group.settings.filter((setting) => setting.spec.advanced);
    if (normal.length > 0) {
      const live = rare.length === 0 ? group.spec.id : null;
      sections.push(settingsSection(group.spec.label, normal, live));
    }
    if (rare.length > 0) {
      // The live sentence goes with the group's last part.
      advanced.push(settingsSection(`Advanced: ${group.spec.label}`, rare, group.spec.id));
    }
    if (pageId === "admission" && group.spec.id === "learn") {
      sections.push({ kind: "extra", id: "what_fits" });
    }
  }
  if (pageId === SERVICE_PAGE) {
    sections.push({ kind: "extra", id: "files" });
  }
  sections.push(...advanced, { kind: "reset", all: pageId === SERVICE_PAGE });
  return sections;
}

/**
 * @param {string} title
 * @param {ValueModel<unknown>[]} settings
 * @param {string | null} live
 * @returns {PageSection}
 */
function settingsSection(title, settings, live) {
  const first = settings[0];
  if (first instanceof SizeTableModel) {
    return { kind: "sizes", title, table: first };
  }
  if (first instanceof RuleListModel) {
    return { kind: "rules", title, rules: first };
  }
  return { kind: "settings", title, settings, live };
}

/**
 * What an allotd page shows above its settings.
 *
 * @typedef {object} PageState
 * @property {SettingsBanner[]} banners
 * @property {string} status The save status, Loading settings…, or empty.
 * @property {boolean} readOnly
 */

/**
 * The Service page's status row.
 *
 * @typedef {object} ServiceRowView
 * @property {string} text
 * @property {string} button The button's title.
 * @property {boolean} canPress
 * @property {boolean} opensLogs Whether Open Logs shows beside it.
 */

/**
 * What Fits on This Machine: a row per class, then the caption.
 *
 * @typedef {object} WhatFitsView
 * @property {Array<{ label: string, text: string }>} rows
 * @property {string} caption
 */

/**
 * What the window model needs from the panel that shows it.
 *
 * @typedef {object} WindowHost
 * @property {(confirmation: Confirmation, onConfirm: () => void) => void} confirm Asks first;
 *   the primary button is destructive.
 * @property {(text: string) => void} notify A brief notice.
 * @property {(path: string) => void} openPath Opens a file or folder in its default app.
 * @property {() => number} now A monotonic clock in milliseconds.
 */

/**
 * @typedef {object} WindowOptions
 * @property {SettingsClient & RestartClient & {
 *   poll: () => Promise<PollResult>,
 *   shutdown: (options?: { record: boolean }) => Promise<string | null>,
 * }} client
 * @property {Timers} timers
 * @property {RestartLauncher} launcher
 * @property {DataPaths} paths
 * @property {number} port The port in `config.toml`, which allotd listens on for HTTP clients.
 * @property {(value: number) => string} number Formats a number for the user's locale.
 * @property {() => Promise<string | null>} readStopRecord
 * @property {WindowHost} host
 * @property {() => Promise<string | null>} [stop] Stops the service, when the panel has its
 *   own way that its popup follows; `client.shutdown()` otherwise.
 * @property {() => Promise<string | null>} [start] Starts it likewise; `launcher.start()`
 *   otherwise.
 */

/**
 * One Settings window's state: its session with allotd, the fit previews, and the service's
 * state as the window knows it. `onChange` fires whenever anything it shows may have changed.
 */
export class SettingsWindowModel {
  session;
  fits;
  number;
  paths;
  /** Whether a GPU shares system memory, so the VRAM meter reads "GPU memory". */
  sharesMemory = false;
  /** @type {(() => void) | null} */
  onChange = null;

  #client;
  #timers;
  #launcher;
  #host;
  #stop;
  #start;
  /** The port in `config.toml`, which allotd listens on for its HTTP clients. */
  #port;
  /** @type {string | null} */
  #version = null;
  #starting = false;
  #startedAt = 0;
  #disposed = false;

  /** @param {WindowOptions} options */
  constructor(options) {
    this.#client = options.client;
    this.#timers = options.timers;
    this.#launcher = options.launcher;
    this.#host = options.host;
    this.#port = options.port;
    this.#stop = options.stop ?? (() => options.client.shutdown());
    this.#start = options.start ?? (() => options.launcher.start());
    this.number = options.number;
    this.paths = options.paths;
    this.session = new SettingsSession(options.client, options.timers, {
      platform: "linux",
      readStopRecord: options.readStopRecord,
    });
    this.fits = new FitPreviews(this.session, options.timers);

    let previousStatus = this.session.status;
    this.session.onChange = () => {
      if (this.session.status !== previousStatus) {
        previousStatus = this.session.status;
        this.#statusChanged();
      }
      this.fits.sessionChanged();
      this.#changed();
    };
    this.fits.onChange = () => this.#changed();
  }

  /** @returns {number} */
  get port() {
    return this.#port;
  }

  /**
   * Fetches now and every 5 s while shown; hiding sends what is pending.
   *
   * @param {boolean} visible
   */
  setVisible(visible) {
    this.session.setVisible(visible);
  }

  /** Stops the timers; a change still pending was sent by `setVisible(false)`. */
  dispose() {
    this.#disposed = true;
    this.session.onChange = null;
    this.fits.onChange = null;
    this.fits.dispose();
    this.session.dispose();
  }

  /**
   * The page now shown: fit previews run only while a page that shows them is open.
   *
   * @param {string} pageId
   */
  showPage(pageId) {
    this.fits.setActive(PAGES_WITH_FITS.includes(pageId));
  }

  /** Fetches the settings again, such as when the window takes focus. */
  async refresh() {
    try {
      await this.session.refresh();
    } catch (error) {
      if (!(error instanceof CancelledError)) {
        throw error;
      }
    }
  }

  /**
   * Whether a GPU shares system memory, from one `GET /ui/state`.
   *
   * @param {MeterToggles} meters
   */
  async readSharesMemory(meters) {
    try {
      const result = await this.#client.poll();
      if (result.state !== null) {
        this.sharesMemory = resourcesView("running", result.state, meters, false).sharesMemory;
        this.#changed();
      }
    } catch (error) {
      if (!(error instanceof CancelledError)) {
        throw error;
      }
    }
  }

  /**
   * @param {string} pageId
   * @returns {PageState}
   */
  pageState(pageId) {
    const session = this.session;
    const save = session.saveStatus;
    // While allotd does not answer, "All changes saved" would say more than is known.
    const quiet = session.status !== "ready" && save.kind === "saved";
    const status = session.status === "loading" ? LOADING_TEXT : quiet ? "" : saveStatusText(save);
    return {
      banners: settingsBanners(session, pageId, this.#port, this.number),
      status,
      readOnly: session.readOnly,
    };
  }

  /**
   * A group's live sentence (DESIGN.md D10), or `null` for a group without one.
   *
   * @param {string} pageId
   * @param {string} groupId
   * @returns {string | null}
   */
  live(pageId, groupId) {
    const number = this.number;
    /** @param {string} key */
    const value = (key) => {
      const model = this.session.setting(key);
      return model instanceof NumberModel ? (model.value ?? 0) : 0;
    };
    switch (`${pageId}/${groupId}`) {
      case "admission/keep_free":
        if (this.fits.unavailable !== null) {
          return this.fits.unavailable;
        }
        return this.fits.latest === null
          ? NOT_YET_TEXT
          : headroomText(this.fits.latest.kept_free, number);
      case "admission/learn": {
        const lower = this.session.setting("estimates.lower");
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
   * An extra line under a setting, such as Port's restart note.
   *
   * @param {string} key
   * @returns {string | null}
   */
  note(key) {
    return restartNote(this.session, key);
  }

  /** @returns {WhatFitsView} */
  get whatFits() {
    const preview = this.fits.unavailable === null ? this.fits.latest : null;
    const classes = this.session.setting("classes");
    const labels = classes instanceof SizeTableModel ? classes.rows : [];
    const rows = (preview?.classes ?? []).map((fit) => {
      const label = labels.find((view) => view.row.name === fit.name)?.label ?? fit.name;
      const now = fitsText(fit.now, fit.min_cores, this.number);
      const idle = fitsText(fit.idle, fit.min_cores, this.number);
      return { label, text: `Now: ${now} · When idle: ${idle}` };
    });
    const caption =
      this.fits.unavailable ??
      (preview === null ? NOT_YET_TEXT : fitsCaption(preview.cpu_busy_pct, this.number));
    return { rows, caption };
  }

  /** @returns {ServiceRowView | null} `null` before allotd first answered. */
  get serviceRow() {
    const status = sessionServiceStatus(this.session.status, this.#starting);
    if (status === null) {
      return null;
    }
    const shown = serviceRow(status, this.session.status, this.#version, this.#port);
    return {
      text: shown.text,
      button: serviceButtonText(shown.button),
      canPress: shown.button !== "starting",
      opensLogs: shown.opensLogs,
    };
  }

  /**
   * The changed settings Reset Page would reset; none while they cannot be changed.
   *
   * @param {string} pageId
   * @returns {ValueModel<unknown>[]}
   */
  changedOn(pageId) {
    return this.session.readOnly ? [] : this.session.changedOn(pageId);
  }

  /** @returns {boolean} Whether Reset All Service Settings… has anything to reset. */
  get canResetAll() {
    return !this.session.readOnly && this.session.changedSettings().length > 0;
  }

  /**
   * Reset Page…, after asking.
   *
   * @param {string} pageId
   * @param {string} label
   */
  resetPage(pageId, label) {
    const changed = this.changedOn(pageId);
    if (changed.length > 0) {
      this.#host.confirm(resetPageConfirmation(label, changed, this.number), () =>
        this.session.resetChanged(pageId));
    }
  }

  /** Reset All Service Settings…, after asking. */
  resetAll() {
    const changed = this.session.changedSettings();
    if (changed.length > 0) {
      this.#host.confirm(resetAllConfirmation(changed.length), () =>
        this.session.resetChanged(null));
    }
  }

  /**
   * A banner's button.
   *
   * @param {BannerAction} action
   */
  act(action) {
    switch (action) {
      case "start_service":
        void this.#startService();
        break;
      case "restart_service":
        void this.#restartService();
        break;
      case "try_now":
      case "check_again":
        void this.refresh();
        break;
      case "open_logs":
        this.openLogs();
        break;
      case "open_file":
        this.openFile();
        break;
      case "apply_mine":
        this.session.applyMine();
        break;
      case "keep_file_values":
        this.session.keepFileValues();
        break;
      case "save_again":
        void this.session.flush();
        break;
    }
  }

  /** The Service row's button: Stop Service… or Start Service. */
  pressService() {
    const status = sessionServiceStatus(this.session.status, this.#starting);
    if (status === "running" || status === "incompatible") {
      void this.#stopService();
    } else if (status === "stopped") {
      void this.#startService();
    }
  }

  openLogs() {
    this.#host.openPath(this.paths.logs);
  }

  openFile() {
    this.#host.openPath(this.paths.config);
  }

  #changed() {
    if (!this.#disposed) {
      this.onChange?.();
    }
  }

  /** The session started or stopped answering: a start under way ends, the version is read. */
  #statusChanged() {
    if (this.session.status === "ready") {
      this.#starting = false;
      this.#client
        .healthVersion()
        .then((version) => {
          this.#version = version;
          this.#changed();
        })
        .catch(() => {});
    }
  }

  /** Stop asks only while leases are active; the stop lasts until a start (M2.1). */
  async #stopService() {
    const poll = await this.#client.poll().catch(() => null);
    const question = stopConfirmation(poll?.state ?? null);
    const stop = async () => {
      const problem = await this.#stop();
      if (problem !== null) {
        this.#host.notify(`The service did not stop: ${problem}`);
      }
      await this.refresh();
    };
    if (question === null) {
      await stop();
      return;
    }
    this.#host.confirm(
      { title: STOP_CONFIRM_TITLE, body: question, button: "Stop Service" },
      () => void stop(),
    );
  }

  async #startService() {
    this.#beginStarting();
    const problem = await this.#start();
    if (problem !== null) {
      this.#starting = false;
      this.#host.notify(`The service did not start: ${problem}`);
    }
    this.#changed();
  }

  /** For a new port or a refused key: through systemd when its unit runs allotd. */
  async #restartService() {
    this.#beginStarting();
    const saved = this.session.setting(PORT_KEY)?.saved;
    const port = typeof saved === "number" ? saved : this.#port;
    const problem = await restartService(this.#client, this.#launcher, this.#timers, port);
    this.#port = port;
    if (problem !== null) {
      this.#starting = false;
      this.#host.notify(`The service did not restart: ${problem}`);
    }
    this.#changed();
  }

  /** Shows Starting… and asks allotd often until it answers or the start times out. */
  #beginStarting() {
    this.#starting = true;
    this.#startedAt = this.#host.now();
    this.#changed();
    const tick = () => {
      if (this.#disposed || !this.#starting) {
        return;
      }
      if (this.#host.now() - this.#startedAt > START_TIMEOUT_MS) {
        this.#starting = false;
        this.#changed();
        return;
      }
      void this.refresh().then(() => this.#timers.setTimeout(tick, START_POLL_MS));
    };
    this.#timers.setTimeout(tick, START_POLL_MS);
  }
}
