// Generated from apps/gnome/allot@mehran.tech/lib/accessible-text.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/** @import { DriveRow, MeterRow } from "./resources-view.js" */
/** @import { JobRow, PanelSnapshot } from "./snapshot.js" */
var __classPrivateFieldSet = (this && this.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
    if (kind === "m") throw new TypeError("Private method is not writable");
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
    return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
};
var __classPrivateFieldGet = (this && this.__classPrivateFieldGet) || function (receiver, state, kind, f) {
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
    return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
};
var _Announcer_summary, _Announcer_spoken;
// The Resources card's width for its label, bar and value columns, in CSS pixels: the popup's 360
// less the card's margins, border, padding and the two gaps between the columns.
const METER_COLUMNS_PX = 310;
const METER_LABEL_PX = 58;
const METER_VALUE_PX = 108;
const METER_BAR_MIN_PX = 48;
/**
 * The widths of a Resources row's outer columns, alike in every row so the bars line up.
 *
 * @typedef {object} MeterColumns
 * @property {number} label
 * @property {number} value
 */
/**
 * What a screen reader reads after a card's action: the rest of the card, which Orca reaches only
 * through flat review because the action is the card's one tab stop.
 *
 * @param {JobRow} row
 * @returns {string}
 */
export function cardDescription(row) {
    const { card, session } = row;
    return [
        session.shortId,
        session.project,
        session.usage,
        row.command,
        card.chip,
        detailText(row),
        card.time,
    ]
        .filter((part) => part !== "")
        .join(", ");
}
/**
 * The detail line in full, with its bold start.
 *
 * @param {JobRow} row
 * @returns {string}
 */
export function detailText(row) {
    return row.card.detailTooltip || row.card.strong + row.card.detail;
}
/**
 * @param {MeterRow} row
 * @returns {string} For example `CPU 64%`.
 */
export function meterName(row) {
    return `${row.label} ${row.value}`;
}
/**
 * @param {DriveRow} row
 * @returns {string} For example `home 612 / 953 GB`.
 */
export function driveName(row) {
    return `${row.label} ${row.value}`;
}
/**
 * The drive's hover label and its activity, which the bar otherwise only shows.
 *
 * @param {DriveRow} row
 * @returns {string} For example `/home, R 42 · W 8 MB/s`.
 */
export function driveDescription(row) {
    return row.activity === "" ? row.id : `${row.id}, ${row.activity}`;
}
/**
 * Column widths for a text size, growing with the text. The value gives way first, so the bar
 * always keeps its minimum.
 *
 * @param {number} textScale The desktop's text scaling factor, 1 at the default size.
 * @returns {MeterColumns}
 */
export function meterColumns(textScale) {
    const scale = Math.max(1, textScale);
    const label = Math.round(METER_LABEL_PX * scale);
    const room = METER_COLUMNS_PX - METER_BAR_MIN_PX - label;
    return { label, value: Math.max(0, Math.min(Math.round(METER_VALUE_PX * scale), room)) };
}
/**
 * Decides what a screen reader announces as snapshots go by while the popup is open: the summary
 * when it changes, and each card's stop outcome once.
 */
export class Announcer {
    constructor() {
        /** @type {string | null} */
        _Announcer_summary.set(this, null);
        /** @type {Map<number, string>} */
        _Announcer_spoken.set(this, new Map());
    }
    /** The popup opened: what it shows now is read by focus, not announced. */
    reset() {
        __classPrivateFieldSet(this, _Announcer_summary, null, "f");
        __classPrivateFieldGet(this, _Announcer_spoken, "f").clear();
    }
    /**
     * @param {PanelSnapshot} snapshot
     * @returns {string[]} What to announce now, in order.
     */
    observe(snapshot) {
        const isFirst = __classPrivateFieldGet(this, _Announcer_summary, "f") === null;
        /** @type {string[]} */
        const texts = [];
        /** @type {Map<number, string>} */
        const spoken = new Map();
        for (const group of snapshot.groups) {
            for (const { id, card } of group.jobs) {
                spoken.set(id, card.announcement);
                if (!isFirst && card.announcement !== "" && card.announcement !== __classPrivateFieldGet(this, _Announcer_spoken, "f").get(id)) {
                    texts.push(card.announcement);
                }
            }
        }
        const { summary } = snapshot.header;
        if (!isFirst && summary !== __classPrivateFieldGet(this, _Announcer_summary, "f")) {
            texts.push(summary);
        }
        __classPrivateFieldSet(this, _Announcer_summary, summary, "f");
        __classPrivateFieldSet(this, _Announcer_spoken, spoken, "f");
        return texts;
    }
}
_Announcer_summary = new WeakMap(), _Announcer_spoken = new WeakMap();
