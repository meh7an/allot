// What the job cards' QML computes beyond its layout, as plain functions a test can reach.

import { QUIET_TICKS } from "../../gnome/allot@mehran.tech/lib/benchmark-status.js";

/** @import { BenchmarkStep } from "../../gnome/allot@mehran.tech/lib/benchmark-status.js" */
/** @import { JobCard } from "../../gnome/allot@mehran.tech/lib/job-card.js" */

/**
 * @typedef {object} Color
 * @property {number} r
 * @property {number} g
 * @property {number} b
 */

/** @typedef {"past" | "current" | "upcoming"} StepState */

/** Breeze icons for the cards, the handoff's dismiss circle, timer and check. */
export const KILL_ICON = "dialog-close";
export const TIMER_ICON = "chronometer";
export const CHECK_ICON = "dialog-ok-apply";

/** The handoff's motion, in milliseconds at Plasma's default animation speed. */
export const ROW_MS = 240;
export const BAR_MS = 900;
export const STEP_RUN_MS = 700;
export const ACTION_GROW_MS = 167;
export const ACTION_SHRINK_MS = 120;

export { QUIET_TICKS };

/** The tile, command and chip of an ended job. */
export const ENDED_OPACITY = 0.5;

// Kirigami.Units.longDuration at Plasma's default animation speed.
const DEFAULT_LONG_DURATION = 200;

const STEP_ORDER = ["drain", "quiet", "run", "done"];

/**
 * A handoff duration at Plasma's animation speed; 0 on its Instant setting.
 *
 * @param {number} ms
 * @param {number} longDuration `Kirigami.Units.longDuration`
 * @returns {number}
 */
export function scaledMs(ms, longDuration) {
  return Math.round((ms * longDuration) / DEFAULT_LONG_DURATION);
}

/**
 * A theme colour at an opacity, as QML reads a colour string.
 *
 * @param {Color} color
 * @param {number} alpha From 0 to 1.
 * @returns {string} `#aarrggbb`
 */
export function withAlpha(color, alpha) {
  const hex = [alpha, color.r, color.g, color.b]
    .map((part) => Math.round(part * 255).toString(16))
    .map((part) => (part.length === 1 ? `0${part}` : part));
  return `#${hex.join("")}`;
}

/**
 * @param {Color} color
 * @returns {boolean} Light text belongs on it.
 */
export function isDark(color) {
  return 0.299 * color.r + 0.587 * color.g + 0.114 * color.b < 0.5;
}

/**
 * The Benchmark purple, which Plasma's colour schemes have no role for.
 *
 * @param {boolean} dark The theme's background is dark.
 * @returns {string}
 */
export function benchmarkColor(dark) {
  return dark ? "#c9a8f0" : "#6f3fb0";
}

/**
 * The text on a solid `benchmarkColor`.
 *
 * @param {boolean} dark
 * @returns {string}
 */
export function benchmarkInk(dark) {
  return dark ? "#1a1a1a" : "#ffffff";
}

/**
 * The detail line as styled text: the bold start, then the rest. Process names and allotd's
 * reasons come from outside, so they are escaped.
 *
 * @param {JobCard} card
 * @returns {string}
 */
export function detailMarkup(card) {
  const strong = card.strong === "" ? "" : `<b>${escapeMarkup(card.strong)}</b>`;
  return strong + escapeMarkup(card.detail);
}

/**
 * A line with an icon is a benchmark status, which wraps instead of truncating.
 *
 * @param {JobCard} card
 * @returns {boolean}
 */
export function detailWraps(card) {
  return card.icon !== "none";
}

/**
 * @param {BenchmarkStep} step
 * @param {BenchmarkStep} current
 * @returns {StepState}
 */
export function stepState(step, current) {
  const index = STEP_ORDER.indexOf(step);
  const reached = STEP_ORDER.indexOf(current);
  if (index === reached) {
    return "current";
  }
  return index < reached ? "past" : "upcoming";
}

/**
 * @param {string} text
 * @returns {string}
 */
function escapeMarkup(text) {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
