import { indicatorStatus } from "./indicator-status.js";
import { JobStops } from "./job-stops.js";
import { resourcesView } from "./resources-view.js";
import { serviceBanner } from "./service-banner.js";
import { defaultSettings } from "./settings.js";
import { panelSnapshot } from "./snapshot.js";
import { StateClock } from "./state-clock.js";

/** @import { PollResult, ServiceStatus } from "./client.js" */
/** @import { IndicatorStatus } from "./indicator-status.js" */
/** @import { StopKind } from "./job-stops.js" */
/** @import { ResourcesView } from "./resources-view.js" */
/** @import { ServiceBanner } from "./service-banner.js" */
/** @import { TraySettings } from "./settings.js" */
/** @import { PanelSnapshot } from "./snapshot.js" */
/** @import { StopResult } from "./stop-result.js" */
/** @import { UiState } from "../contract.g.mjs" */

/**
 * How the popup behaves. GNOME keeps the shell's own UI above app windows, so Stay open and
 * Pin both float; Pin adds dragging.
 *
 * - `light-dismiss`: holds the pointer and keyboard; closes on a click elsewhere or Esc.
 * - `stay-open`: lets go of input and stays under the panel button until closed from it.
 * - `pinned`: Stay open, draggable by its header to anywhere on any monitor.
 *
 * @typedef {"light-dismiss" | "stay-open" | "pinned"} PopupMode
 */

/**
 * What changed in one update, so the UI rebuilds only what it must.
 *
 * @typedef {object} ModelChange
 * @property {boolean} snapshot
 *   Anything the popup shows: the snapshot, the Resources card or the service banner.
 * @property {boolean} indicator
 */

/**
 * Ends leases on the user's behalf: `DaemonClient`, or a fake for the tests and scenarios.
 * Both calls resolve with allotd's answer and reject only with `CancelledError`.
 *
 * @typedef {object} JobStopper
 * @property {(lease: number) => Promise<StopResult>} kill
 * @property {(lease: number) => Promise<StopResult>} remove
 */

/**
 * The state the popup and the panel button render: the latest snapshot, the Resources card, the
 * service banner, the settings, the popup's mode, and the Kill and Remove of each card.
 */
export class PopupModel {
  /** @type {PanelSnapshot} */
  snapshot;

  /** @type {IndicatorStatus} */
  indicator;

  /** @type {ResourcesView} */
  resources;

  /**
   * The banner while the service is down, else `null`.
   *
   * @type {ServiceBanner | null}
   */
  banner = null;

  /** @type {TraySettings} */
  settings = defaultSettings();

  /** @type {PopupMode} */
  mode = "light-dismiss";

  /**
   * GNOME's Do Not Disturb hides banners, which the menu and the first-run card say.
   *
   * @type {boolean}
   */
  doNotDisturb = false;

  /**
   * A daemon time as the user's short time of day, for the banner's caption.
   *
   * @type {(ms: number) => string}
   */
  timeOfDay = (ms) => new Date(ms).toLocaleTimeString();

  /**
   * Called after every change of `settings`, so the caller can save them.
   *
   * @type {((settings: TraySettings) => void) | null}
   */
  onSettingsChanged = null;

  /**
   * Called right before a Kill goes to allotd, so the notification rules know it is ours.
   *
   * @type {((lease: number) => void) | null}
   */
  onKillSent = null;

  /**
   * Sends Kill and Remove; without one, presses only change the cards.
   *
   * @type {JobStopper | null}
   */
  stopper = null;

  /**
   * Called after a change that no poll or press caused: allotd answered a Kill or Remove.
   *
   * @type {((change: ModelChange) => void) | null}
   */
  onChange = null;

  /**
   * Called when allotd has answered a Kill or Remove, so the caller can poll at once.
   *
   * @type {((lease: number, kind: StopKind, result: StopResult) => void) | null}
   */
  onStopAnswered = null;

  #stops;
  #clock;
  /** @type {ServiceStatus} */
  #status = "starting";
  /** @type {UiState | null} */
  #shown = null;
  /** @type {UiState | null} */
  #lastKnown = null;
  /** @type {string | null} */
  #startProblem = null;
  #snapshotKey;
  #indicatorKey;

  /**
   * Until the first poll answers, the service is assumed to be starting.
   *
   * @param {() => number} [now] The local clock in milliseconds that ticks times and stops; the
   *   tests pass their own.
   */
  constructor(now = () => Date.now()) {
    this.#stops = new JobStops(now);
    this.#clock = new StateClock(now);
    this.snapshot = panelSnapshot("starting", null);
    this.indicator = indicatorStatus(this.snapshot);
    this.resources = resourcesView("starting", null, this.settings.meters, false);
    this.#snapshotKey = this.#shownKey();
    this.#indicatorKey = JSON.stringify(this.indicator);
  }

  /**
   * Shows a poll; while allotd is not running, the jobs come from `lastKnown`.
   *
   * @param {PollResult} result
   * @param {UiState | null} lastKnown
   * @param {string | null} [startProblem] Why the last Start Service failed, for the banner.
   * @returns {ModelChange}
   */
  apply(result, lastKnown, startProblem = null) {
    this.#status = result.status;
    this.#shown = result.state ?? lastKnown;
    this.#lastKnown = lastKnown;
    this.#startProblem = startProblem;
    this.#clock.received(result.state);
    if (result.state === null) {
      this.#stops.cancelConfirms();
    } else {
      this.#stops.observe(result.state);
    }
    return this.refresh();
  }

  /**
   * Rebuilds the snapshot for the current moment, so times tick between polls.
   *
   * @returns {ModelChange}
   */
  refresh() {
    this.#stops.expire();
    const shown = this.#shown;
    const live = this.#status === "running" && shown !== null;
    const next = panelSnapshot(this.#status, shown, {
      ...(live ? { jobs: this.#stops.merge(shown) } : {}),
      stops: this.#stops.views,
      nowMs: this.#clock.nowMs,
      firstRun: { isDone: this.settings.first_run_done, doNotDisturb: this.doNotDisturb },
    });
    this.snapshot = next;
    this.resources = resourcesView(
      this.#status,
      shown,
      this.settings.meters,
      this.settings.resources_expanded,
    );
    this.banner = serviceBanner(
      this.#status,
      this.#lastKnown,
      this.#startProblem,
      this.timeOfDay,
    );
    // Updating St actors for an unchanged state would churn for nothing.
    const snapshotKey = this.#shownKey();
    if (snapshotKey === this.#snapshotKey) {
      return { snapshot: false, indicator: false };
    }
    this.#snapshotKey = snapshotKey;

    const indicator = indicatorStatus(next);
    const indicatorKey = JSON.stringify(indicator);
    if (indicatorKey === this.#indicatorKey) {
      return { snapshot: true, indicator: false };
    }
    this.indicator = indicator;
    this.#indicatorKey = indicatorKey;
    return { snapshot: true, indicator: true };
  }

  /**
   * The status of the latest poll, or `starting` after a press of Start Service.
   *
   * @returns {ServiceStatus}
   */
  get status() {
    return this.#status;
  }

  /**
   * Replaces the settings as read at startup, without saving them again.
   *
   * @param {TraySettings} settings
   * @returns {ModelChange}
   */
  loadSettings(settings) {
    this.settings = settings;
    return this.refresh();
  }

  /**
   * Changes the settings, shows the change and hands them to be saved.
   *
   * @param {(settings: TraySettings) => void} change
   * @returns {ModelChange}
   */
  updateSettings(change) {
    const next = JSON.parse(JSON.stringify(this.settings));
    change(next);
    if (JSON.stringify(next) === JSON.stringify(this.settings)) {
      return { snapshot: false, indicator: false };
    }
    this.settings = next;
    const shown = this.refresh();
    this.onSettingsChanged?.(next);
    return shown;
  }

  /**
   * The Resources card's header: expanded shows the rows, collapsed the summary.
   *
   * @returns {ModelChange}
   */
  toggleResources() {
    return this.updateSettings((settings) => {
      settings.resources_expanded = !settings.resources_expanded;
    });
  }

  /**
   * The first-run card's Done.
   *
   * @returns {ModelChange}
   */
  finishFirstRun() {
    return this.updateSettings((settings) => {
      settings.first_run_done = true;
    });
  }

  /**
   * The first-run card shows for the first time ever: the popup should open by itself, once.
   *
   * @returns {boolean}
   */
  get wantsFirstOpen() {
    return this.snapshot.welcome !== null && !this.settings.first_run_shown;
  }

  /**
   * The popup opened by itself, which it does not do again. Noted at the opening itself, so a
   * shell that stops before it can open tries again next time.
   *
   * @returns {ModelChange}
   */
  noteFirstOpened() {
    return this.updateSettings((settings) => {
      settings.first_run_shown = true;
    });
  }

  /**
   * Shows whether GNOME's Do Not Disturb is on, in the menu and the first-run card.
   *
   * @param {boolean} on
   * @returns {ModelChange}
   */
  setDoNotDisturb(on) {
    this.doNotDisturb = on;
    return this.refresh();
  }

  /**
   * Start Service was pressed: the banner shows Starting… until the next poll says otherwise.
   *
   * @returns {ModelChange}
   */
  noteStarting() {
    this.#status = "starting";
    this.#startProblem = null;
    return this.refresh();
  }

  /**
   * The milliseconds until a card changes by itself (a countdown second or a hold ending), or
   * `null`.
   *
   * @returns {number | null}
   */
  get nextStopChangeMs() {
    return this.#stops.nextChangeMs;
  }

  /**
   * Whether a Kill? confirm is up, which Esc cancels before it closes the popup.
   *
   * @returns {boolean}
   */
  get isConfirming() {
    return this.#stops.isConfirming;
  }

  /**
   * A press of Kill, or of Kill? within its countdown; read `snapshot` afterwards.
   *
   * @param {number} lease
   * @returns {Promise<void> | null} The request to allotd when the press sends one, for the
   *   tests to await.
   */
  pressKill(lease) {
    const press = this.#stops.pressKill(lease);
    this.refresh();
    return press === "send" ? this.#send(lease, "kill") : null;
  }

  /**
   * A press of Remove on a queued request; read `snapshot` afterwards.
   *
   * @param {number} lease
   * @returns {Promise<void> | null} The request to allotd when the press sends one.
   */
  pressRemove(lease) {
    const sends = this.#stops.pressRemove(lease);
    this.refresh();
    return sends ? this.#send(lease, "remove") : null;
  }

  /**
   * Focus left a Kill? confirm: it goes back to Kill.
   *
   * @param {number} lease
   * @returns {boolean} True when the confirm was cancelled.
   */
  cancelConfirm(lease) {
    if (!this.#stops.cancel(lease)) {
      return false;
    }
    this.refresh();
    return true;
  }

  /**
   * Esc, or the popup closing: every Kill? goes back to Kill.
   *
   * @returns {boolean} True when any confirm was cancelled.
   */
  cancelConfirms() {
    if (!this.#stops.cancelConfirms()) {
      return false;
    }
    this.refresh();
    return true;
  }

  /** Opening from the panel always starts in light-dismiss mode. */
  resetForOpen() {
    this.mode = "light-dismiss";
  }

  /** Stay open on, or off; turning it off also unpins. */
  toggleStayOpen() {
    this.mode = this.mode === "light-dismiss" ? "stay-open" : "light-dismiss";
  }

  /** Pin exists only while Stay open is on. */
  togglePinned() {
    switch (this.mode) {
      case "light-dismiss":
        break;
      case "stay-open":
        this.mode = "pinned";
        break;
      case "pinned":
        this.mode = "stay-open";
        break;
    }
  }

  /** @returns {boolean} */
  get isStayOpen() {
    return this.mode !== "light-dismiss";
  }

  /** @returns {boolean} */
  get isPinned() {
    return this.mode === "pinned";
  }

  /**
   * Everything the popup shows, to compare with the last time.
   *
   * @returns {string}
   */
  #shownKey() {
    return JSON.stringify([this.snapshot, this.resources, this.banner]);
  }

  /**
   * @param {number} lease
   * @param {"kill" | "remove"} kind
   * @returns {Promise<void> | null}
   */
  #send(lease, kind) {
    const stopper = this.stopper;
    if (stopper === null) {
      return null;
    }
    if (kind === "kill") {
      this.onKillSent?.(lease);
    }
    const request = kind === "kill" ? stopper.kill(lease) : stopper.remove(lease);
    // The second handler ignores a rejection: only the extension being disabled cancels a stop.
    return request.then(
      (answer) => {
        this.#stops.finished(lease, answer);
        const change = this.refresh();
        this.onChange?.(change);
        this.onStopAnswered?.(lease, kind, answer);
      },
      () => {},
    );
  }
}
