// Generated from apps/kde/src/card-view.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
// What the job cards' QML computes beyond its layout, as plain functions a test can reach.
import { cardDescription, driveDescription, driveName, meterName, } from "./shared/accessible-text.js";
import { barMove } from "./shared/bar-move.js";
import { QUIET_TICKS } from "./shared/benchmark-status.js";
/** @import { BenchmarkStep } from "./shared/benchmark-status.js" */
/** @import { JobCard } from "./shared/job-card.js" */
/** @import { DriveRow, MeterRow } from "./shared/resources-view.js" */
/**
 * @typedef {object} Color
 * @property {number} r
 * @property {number} g
 * @property {number} b
 */
/** @typedef {"past" | "current" | "upcoming"} StepState */
/** Breeze icons for the cards, the handoff's dismiss circle, timer and check. */
export { barMove };
export const KILL_ICON = "dialog-close";
export const TIMER_ICON = "chronometer";
export const CHECK_ICON = "dialog-ok-apply";
/** The handoff's motion, in milliseconds at Plasma's default animation speed. */
export const ROW_MS = 240;
export const BAR_MS = 900;
export const STEP_RUN_MS = 700;
export const ACTION_GROW_MS = 167;
export const ACTION_SHRINK_MS = 120;
export const FOLD_MS = 240;
export const SUMMARY_MS = 150;
export const GLIDE_MS = 600;
/** The Resources card's header arrow: up opens the card, which is docked at the popup's bottom. */
export const FOLD_UP_ICON = "arrow-up";
export const FOLD_DOWN_ICON = "arrow-down";
/** One line, as the handoff has it: the fold is as tall as its rows. */
export const ALL_OFF_TEXT = "All meters are off. Turn them on from ⋯ › Meters.";
/**
 * A row that has just left the Resources card, for the moment before its view does.
 *
 * @type {MeterRow & DriveRow}
 */
const GONE_ROW = {
    id: "",
    label: "",
    used: 0,
    shared: null,
    reserved: null,
    headroom: 1,
    value: "",
    detail: "",
    spark: [],
    activity: "",
};
export { QUIET_TICKS, cardDescription, driveDescription, driveName, meterName };
/** The tile, command and chip of an ended job. */
export const ENDED_OPACITY = 0.5;
// Kirigami.Units.longDuration at Plasma's default animation speed.
const DEFAULT_LONG_DURATION = 200;
const STEP_ORDER = ["drain", "quiet", "run", "done"];
// Plasma has no high-contrast switch, only colour schemes. Breeze's own reach about 18 : 1, and a
// scheme of near-pure black and white about 20 or 21.
const HIGH_CONTRAST_RATIO = 19;
const HIGH_CONTRAST_TRACK_ALPHA = 0.35;
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
 * The WCAG contrast ratio between two colours.
 *
 * @param {Color} first
 * @param {Color} second
 * @returns {number} From 1 to 21.
 */
export function contrastRatio(first, second) {
    const lighter = Math.max(luminance(first), luminance(second));
    const darker = Math.min(luminance(first), luminance(second));
    return (lighter + 0.05) / (darker + 0.05);
}
/**
 * @param {Color} text
 * @param {Color} background
 * @returns {boolean} The colour scheme asks for high contrast.
 */
export function isHighContrast(text, background) {
    return contrastRatio(text, background) >= HIGH_CONTRAST_RATIO;
}
/**
 * A card's or tile's edge: faint normally, solid in high contrast, where nothing else shows the
 * card's shape.
 *
 * @param {Color} text The theme's text colour.
 * @param {Color} background
 * @param {number} alpha The handoff's opacity.
 * @returns {string} `#aarrggbb`
 */
export function edgeColor(text, background, alpha) {
    return withAlpha(text, isHighContrast(text, background) ? 1 : alpha);
}
/**
 * A bar's track: stronger in high contrast, but short of the fill's own colour.
 *
 * @param {Color} text The theme's text colour.
 * @param {Color} background
 * @param {number} alpha The handoff's opacity.
 * @returns {string} `#aarrggbb`
 */
export function trackColor(text, background, alpha) {
    return withAlpha(text, isHighContrast(text, background) ? HIGH_CONTRAST_TRACK_ALPHA : alpha);
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
 * The Resources card's row ids as one string. QML rebuilds a Repeater whenever its model is
 * assigned, and a new array comes with every poll; a string binding fires only when it changes.
 *
 * @param {{ id: string }[]} rows
 * @returns {string}
 */
export function rowIds(rows) {
    return rows.map((row) => row.id).join("\n");
}
/**
 * The views' model for `rowIds`.
 *
 * @param {string} ids
 * @returns {string[]}
 */
export function splitIds(ids) {
    return ids === "" ? [] : ids.split("\n");
}
/**
 * @param {(MeterRow | DriveRow)[]} rows
 * @param {string} id
 * @returns {MeterRow & DriveRow} A view reads the fields of its own kind of row.
 */
export function rowById(rows, id) {
    const found = rows.find((row) => row.id === id);
    return /** @type {MeterRow & DriveRow} */ (found !== null && found !== void 0 ? found : GONE_ROW);
}
/**
 * A Resources row's outer columns, alike in every row so the bars line up. They grow with the text
 * from the handoff's widths, and the value gives way first, so the bar keeps its minimum.
 *
 * @param {object} sizes In pixels.
 * @param {number} sizes.minLabel The handoff's label column.
 * @param {number} sizes.minValue The handoff's value column.
 * @param {number} sizes.minBar The narrowest bar.
 * @param {number} sizes.labelText The widest label at the current font.
 * @param {number} sizes.valueText The widest value at the current font.
 * @param {number} sizes.room The row's width less the gaps between its columns.
 * @returns {{ label: number, value: number }}
 */
export function meterColumns(sizes) {
    const label = Math.max(sizes.minLabel, sizes.labelText);
    const wanted = Math.max(sizes.minValue, sizes.valueText);
    return { label, value: Math.max(0, Math.min(wanted, sizes.room - label - sizes.minBar)) };
}
/**
 * The sparkline's points, oldest at the left, with the bottom for 0.
 *
 * @param {number[]} samples Fractions.
 * @param {number} width
 * @param {number} height
 * @returns {{ x: number, y: number }[]}
 */
export function sparkPoints(samples, width, height) {
    const steps = Math.max(1, samples.length - 1);
    return samples.map((sample, index) => ({
        x: (index * width) / steps,
        y: (1 - sample) * height,
    }));
}
/**
 * @param {Color} color
 * @returns {number} WCAG relative luminance.
 */
function luminance(color) {
    /** @param {number} part */
    const linear = (part) => (part <= 0.03928 ? part / 12.92 : Math.pow((part + 0.055) / 1.055, 2.4));
    return 0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b);
}
/**
 * @param {string} text
 * @returns {string}
 */
function escapeMarkup(text) {
    return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}
