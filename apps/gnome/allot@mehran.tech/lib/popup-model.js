import { indicatorStatus } from "./indicator-status.js";
import { JobStops } from "./job-stops.js";
import { panelSnapshot } from "./snapshot.js";
import { StateClock } from "./state-clock.js";

/** @import { PollResult, ServiceStatus } from "./client.js" */
/** @import { IndicatorStatus } from "./indicator-status.js" */
/** @import { StopKind } from "./job-stops.js" */
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
 * The state the popup and the panel button render: the latest snapshot, the popup's mode, and
 * the Kill and Remove of each card.
 */
export class PopupModel {
  /** @type {PanelSnapshot} */
  snapshot;

  /** @type {IndicatorStatus} */
  indicator;

  /** @type {PopupMode} */
  mode = "light-dismiss";

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
    this.#snapshotKey = JSON.stringify(this.snapshot);
    this.#indicatorKey = JSON.stringify(this.indicator);
  }

  /**
   * Shows a poll; while allotd is not running, the jobs come from `lastKnown`.
   *
   * @param {PollResult} result
   * @param {UiState | null} lastKnown
   * @returns {ModelChange}
   */
  apply(result, lastKnown) {
    this.#status = result.status;
    this.#shown = result.state ?? lastKnown;
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
    });
    // Rebuilding St actors for an unchanged state would churn for nothing.
    const snapshotKey = JSON.stringify(next);
    if (snapshotKey === this.#snapshotKey) {
      return { snapshot: false, indicator: false };
    }
    this.snapshot = next;
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
   * @param {number} lease
   * @param {"kill" | "remove"} kind
   * @returns {Promise<void> | null}
   */
  #send(lease, kind) {
    const stopper = this.stopper;
    if (stopper === null) {
      return null;
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
