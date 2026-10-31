/** @import { PanelSnapshot } from "./snapshot.js" */

export const APP_NAME = "Allot";

/**
 * The panel icon's state, one symbolic icon each.
 *
 * @typedef {"idle" | "busy" | "benchmark" | "queued" | "down"} IndicatorIcon
 */

/** @type {readonly IndicatorIcon[]} */
export const INDICATOR_ICONS = ["idle", "busy", "benchmark", "queued", "down"];

/**
 * What the panel button shows for one snapshot: the icon and the summary its accessible name
 * repeats.
 *
 * @typedef {object} IndicatorStatus
 * @property {IndicatorIcon} icon
 * @property {string} summary
 */

/**
 * The icon's file under the extension's `icons` folder, written by `assets/brand/build.py`.
 * The `-symbolic` suffix makes the shell recolour it to the panel's text colour.
 *
 * @param {IndicatorIcon} icon
 * @returns {string}
 */
export function iconFile(icon) {
  return `allot-${icon}-symbolic.svg`;
}

/**
 * @param {IndicatorStatus} status
 * @returns {string}
 */
export function accessibleName(status) {
  return `${APP_NAME} · ${status.summary}`;
}

/**
 * Down while allotd is not serving state; Benchmark while a benchmark drains, waits for quiet
 * or runs alone; Queued while anything else waits; Busy while a job runs; Idle otherwise. The
 * same order as the Windows tray icon and the macOS menu bar.
 *
 * @param {PanelSnapshot} snapshot
 * @returns {IndicatorStatus}
 */
export function indicatorStatus(snapshot) {
  const isWaiting = snapshot.groups.some((group) => group.jobs.some((job) => job.isQueued));
  /** @type {IndicatorIcon} */
  let icon = "idle";
  if (snapshot.isLastKnown) {
    icon = "down";
  } else if (snapshot.benchmark !== null) {
    icon = "benchmark";
  } else if (isWaiting) {
    icon = "queued";
  } else if (snapshot.header.glyph === "busy") {
    icon = "busy";
  }
  return { icon, summary: snapshot.header.summary };
}
