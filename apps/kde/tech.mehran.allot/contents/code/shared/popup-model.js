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
var _PopupModel_instances, _PopupModel_stops, _PopupModel_clock, _PopupModel_status, _PopupModel_shown, _PopupModel_snapshotKey, _PopupModel_indicatorKey, _PopupModel_send;
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
    /**
     * Until the first poll answers, the service is assumed to be starting.
     *
     * @param {() => number} [now] The local clock in milliseconds that ticks times and stops; the
     *   tests pass their own.
     */
    constructor(now = () => Date.now()) {
        _PopupModel_instances.add(this);
        /** @type {PopupMode} */
        this.mode = "light-dismiss";
        /**
         * Sends Kill and Remove; without one, presses only change the cards.
         *
         * @type {JobStopper | null}
         */
        this.stopper = null;
        /**
         * Called after a change that no poll or press caused: allotd answered a Kill or Remove.
         *
         * @type {((change: ModelChange) => void) | null}
         */
        this.onChange = null;
        /**
         * Called when allotd has answered a Kill or Remove, so the caller can poll at once.
         *
         * @type {((lease: number, kind: StopKind, result: StopResult) => void) | null}
         */
        this.onStopAnswered = null;
        _PopupModel_stops.set(this, void 0);
        _PopupModel_clock.set(this, void 0);
        /** @type {ServiceStatus} */
        _PopupModel_status.set(this, "starting");
        /** @type {UiState | null} */
        _PopupModel_shown.set(this, null);
        _PopupModel_snapshotKey.set(this, void 0);
        _PopupModel_indicatorKey.set(this, void 0);
        __classPrivateFieldSet(this, _PopupModel_stops, new JobStops(now), "f");
        __classPrivateFieldSet(this, _PopupModel_clock, new StateClock(now), "f");
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
        __classPrivateFieldSet(this, _PopupModel_status, result.status, "f");
        __classPrivateFieldSet(this, _PopupModel_shown, (_a = result.state) !== null && _a !== void 0 ? _a : lastKnown, "f");
        __classPrivateFieldGet(this, _PopupModel_clock, "f").received(result.state);
        if (result.state === null) {
            __classPrivateFieldGet(this, _PopupModel_stops, "f").cancelConfirms();
        }
        else {
            __classPrivateFieldGet(this, _PopupModel_stops, "f").observe(result.state);
        }
        return this.refresh();
    }
    /**
     * Rebuilds the snapshot for the current moment, so times tick between polls.
     *
     * @returns {ModelChange}
     */
    refresh() {
        __classPrivateFieldGet(this, _PopupModel_stops, "f").expire();
        const shown = __classPrivateFieldGet(this, _PopupModel_shown, "f");
        const live = __classPrivateFieldGet(this, _PopupModel_status, "f") === "running" && shown !== null;
        const next = panelSnapshot(__classPrivateFieldGet(this, _PopupModel_status, "f"), shown, Object.assign(Object.assign({}, (live ? { jobs: __classPrivateFieldGet(this, _PopupModel_stops, "f").merge(shown) } : {})), { stops: __classPrivateFieldGet(this, _PopupModel_stops, "f").views, nowMs: __classPrivateFieldGet(this, _PopupModel_clock, "f").nowMs }));
        // Rebuilding St actors for an unchanged state would churn for nothing.
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
    /**
     * The milliseconds until a card changes by itself (a countdown second or a hold ending), or
     * `null`.
     *
     * @returns {number | null}
     */
    get nextStopChangeMs() {
        return __classPrivateFieldGet(this, _PopupModel_stops, "f").nextChangeMs;
    }
    /**
     * Whether a Kill? confirm is up, which Esc cancels before it closes the popup.
     *
     * @returns {boolean}
     */
    get isConfirming() {
        return __classPrivateFieldGet(this, _PopupModel_stops, "f").isConfirming;
    }
    /**
     * A press of Kill, or of Kill? within its countdown; read `snapshot` afterwards.
     *
     * @param {number} lease
     * @returns {Promise<void> | null} The request to allotd when the press sends one, for the
     *   tests to await.
     */
    pressKill(lease) {
        const press = __classPrivateFieldGet(this, _PopupModel_stops, "f").pressKill(lease);
        this.refresh();
        return press === "send" ? __classPrivateFieldGet(this, _PopupModel_instances, "m", _PopupModel_send).call(this, lease, "kill") : null;
    }
    /**
     * A press of Remove on a queued request; read `snapshot` afterwards.
     *
     * @param {number} lease
     * @returns {Promise<void> | null} The request to allotd when the press sends one.
     */
    pressRemove(lease) {
        const sends = __classPrivateFieldGet(this, _PopupModel_stops, "f").pressRemove(lease);
        this.refresh();
        return sends ? __classPrivateFieldGet(this, _PopupModel_instances, "m", _PopupModel_send).call(this, lease, "remove") : null;
    }
    /**
     * Focus left a Kill? confirm: it goes back to Kill.
     *
     * @param {number} lease
     * @returns {boolean} True when the confirm was cancelled.
     */
    cancelConfirm(lease) {
        if (!__classPrivateFieldGet(this, _PopupModel_stops, "f").cancel(lease)) {
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
        if (!__classPrivateFieldGet(this, _PopupModel_stops, "f").cancelConfirms()) {
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
}
_PopupModel_stops = new WeakMap(), _PopupModel_clock = new WeakMap(), _PopupModel_status = new WeakMap(), _PopupModel_shown = new WeakMap(), _PopupModel_snapshotKey = new WeakMap(), _PopupModel_indicatorKey = new WeakMap(), _PopupModel_instances = new WeakSet(), _PopupModel_send = function _PopupModel_send(lease, kind) {
    const stopper = this.stopper;
    if (stopper === null) {
        return null;
    }
    const request = kind === "kill" ? stopper.kill(lease) : stopper.remove(lease);
    // The second handler ignores a rejection: only the extension being disabled cancels a stop.
    return request.then((answer) => {
        var _a, _b;
        __classPrivateFieldGet(this, _PopupModel_stops, "f").finished(lease, answer);
        const change = this.refresh();
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this, change);
        (_b = this.onStopAnswered) === null || _b === void 0 ? void 0 : _b.call(this, lease, kind, answer);
    }, () => { });
};
