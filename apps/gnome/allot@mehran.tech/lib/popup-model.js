import { indicatorStatus } from "./indicator-status.js";
import { panelSnapshot } from "./snapshot.js";

/** @import { PollResult } from "./client.js" */
/** @import { IndicatorStatus } from "./indicator-status.js" */
/** @import { PanelSnapshot } from "./snapshot.js" */
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
 * What changed in one `apply`, so the UI rebuilds only what it must.
 *
 * @typedef {object} ModelChange
 * @property {boolean} snapshot
 * @property {boolean} indicator
 */

/**
 * The state the popup and the panel button render: the latest snapshot and the popup's mode.
 */
export class PopupModel {
  /** @type {PanelSnapshot} */
  snapshot;

  /** @type {IndicatorStatus} */
  indicator;

  /** @type {PopupMode} */
  mode = "light-dismiss";

  #snapshotKey;
  #indicatorKey;

  /** Until the first poll answers, the service is assumed to be starting. */
  constructor() {
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
    const next = panelSnapshot(result.status, result.state ?? lastKnown);
    // Rebuilding St actors every second would churn for nothing on an unchanged state.
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
}
