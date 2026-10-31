// Generated from apps/gnome/allot@mehran.tech/lib/popup-model.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
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
var _PopupModel_snapshotKey, _PopupModel_indicatorKey;
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
    /** Until the first poll answers, the service is assumed to be starting. */
    constructor() {
        /** @type {PopupMode} */
        this.mode = "light-dismiss";
        _PopupModel_snapshotKey.set(this, void 0);
        _PopupModel_indicatorKey.set(this, void 0);
        this.snapshot = panelSnapshot("starting", null);
        this.indicator = indicatorStatus(this.snapshot);
        __classPrivateFieldSet(this, _PopupModel_snapshotKey, JSON.stringify(this.snapshot), "f");
        __classPrivateFieldSet(this, _PopupModel_indicatorKey, JSON.stringify(this.indicator), "f");
    }
    /**
     * Shows a poll; while allotd is not running, the jobs come from `lastKnown`.
     *
     * @param {PollResult} result
     * @param {UiState | null} lastKnown
     * @returns {ModelChange}
     */
    apply(result, lastKnown) {
        var _a;
        const next = panelSnapshot(result.status, (_a = result.state) !== null && _a !== void 0 ? _a : lastKnown);
        // Rebuilding St actors every second would churn for nothing on an unchanged state.
        const snapshotKey = JSON.stringify(next);
        if (snapshotKey === __classPrivateFieldGet(this, _PopupModel_snapshotKey, "f")) {
            return { snapshot: false, indicator: false };
        }
        this.snapshot = next;
        __classPrivateFieldSet(this, _PopupModel_snapshotKey, snapshotKey, "f");
        const indicator = indicatorStatus(next);
        const indicatorKey = JSON.stringify(indicator);
        if (indicatorKey === __classPrivateFieldGet(this, _PopupModel_indicatorKey, "f")) {
            return { snapshot: true, indicator: false };
        }
        this.indicator = indicator;
        __classPrivateFieldSet(this, _PopupModel_indicatorKey, indicatorKey, "f");
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
_PopupModel_snapshotKey = new WeakMap(), _PopupModel_indicatorKey = new WeakMap();
