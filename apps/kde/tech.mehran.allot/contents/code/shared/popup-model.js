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
var _PopupModel_instances, _PopupModel_stops, _PopupModel_clock, _PopupModel_status, _PopupModel_shown, _PopupModel_lastKnown, _PopupModel_startProblem, _PopupModel_snapshotKey, _PopupModel_indicatorKey, _PopupModel_shownKey, _PopupModel_send;
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
    /**
     * Until the first poll answers, the service is assumed to be starting.
     *
     * @param {() => number} [now] The local clock in milliseconds that ticks times and stops; the
     *   tests pass their own.
     */
    constructor(now = () => Date.now()) {
        _PopupModel_instances.add(this);
        /**
         * The banner while the service is down, else `null`.
         *
         * @type {ServiceBanner | null}
         */
        this.banner = null;
        /** @type {TraySettings} */
        this.settings = defaultSettings();
        /** @type {PopupMode} */
        this.mode = "light-dismiss";
        /**
         * GNOME's Do Not Disturb hides banners, which the menu and the first-run card say.
         *
         * @type {boolean}
         */
        this.doNotDisturb = false;
        /**
         * A daemon time as the user's short time of day, for the banner's caption.
         *
         * @type {(ms: number) => string}
         */
        this.timeOfDay = (ms) => new Date(ms).toLocaleTimeString();
        /**
         * Called after every change of `settings`, so the caller can save them.
         *
         * @type {((settings: TraySettings) => void) | null}
         */
        this.onSettingsChanged = null;
        /**
         * Called right before a Kill goes to allotd, so the notification rules know it is ours.
         *
         * @type {((lease: number) => void) | null}
         */
        this.onKillSent = null;
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
        /** @type {UiState | null} */
        _PopupModel_lastKnown.set(this, null);
        /** @type {string | null} */
        _PopupModel_startProblem.set(this, null);
        _PopupModel_snapshotKey.set(this, void 0);
        _PopupModel_indicatorKey.set(this, void 0);
        __classPrivateFieldSet(this, _PopupModel_stops, new JobStops(now), "f");
        __classPrivateFieldSet(this, _PopupModel_clock, new StateClock(now), "f");
        this.snapshot = panelSnapshot("starting", null);
        this.indicator = indicatorStatus(this.snapshot);
        this.resources = resourcesView("starting", null, this.settings.meters, false);
        __classPrivateFieldSet(this, _PopupModel_snapshotKey, __classPrivateFieldGet(this, _PopupModel_instances, "m", _PopupModel_shownKey).call(this), "f");
        __classPrivateFieldSet(this, _PopupModel_indicatorKey, JSON.stringify(this.indicator), "f");
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
        var _a;
        __classPrivateFieldSet(this, _PopupModel_status, result.status, "f");
        __classPrivateFieldSet(this, _PopupModel_shown, (_a = result.state) !== null && _a !== void 0 ? _a : lastKnown, "f");
        __classPrivateFieldSet(this, _PopupModel_lastKnown, lastKnown, "f");
        __classPrivateFieldSet(this, _PopupModel_startProblem, startProblem, "f");
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
        const next = panelSnapshot(__classPrivateFieldGet(this, _PopupModel_status, "f"), shown, Object.assign(Object.assign({}, (live ? { jobs: __classPrivateFieldGet(this, _PopupModel_stops, "f").merge(shown) } : {})), { stops: __classPrivateFieldGet(this, _PopupModel_stops, "f").views, nowMs: __classPrivateFieldGet(this, _PopupModel_clock, "f").nowMs, firstRun: { isDone: this.settings.first_run_done, doNotDisturb: this.doNotDisturb } }));
        this.snapshot = next;
        this.resources = resourcesView(__classPrivateFieldGet(this, _PopupModel_status, "f"), shown, this.settings.meters, this.settings.resources_expanded);
        this.banner = serviceBanner(__classPrivateFieldGet(this, _PopupModel_status, "f"), __classPrivateFieldGet(this, _PopupModel_lastKnown, "f"), __classPrivateFieldGet(this, _PopupModel_startProblem, "f"), this.timeOfDay);
        // Updating St actors for an unchanged state would churn for nothing.
        const snapshotKey = __classPrivateFieldGet(this, _PopupModel_instances, "m", _PopupModel_shownKey).call(this);
        if (snapshotKey === __classPrivateFieldGet(this, _PopupModel_snapshotKey, "f")) {
            return { snapshot: false, indicator: false };
        }
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
     * The status of the latest poll, or `starting` after a press of Start Service.
     *
     * @returns {ServiceStatus}
     */
    get status() {
        return __classPrivateFieldGet(this, _PopupModel_status, "f");
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
        var _a;
        const next = JSON.parse(JSON.stringify(this.settings));
        change(next);
        if (JSON.stringify(next) === JSON.stringify(this.settings)) {
            return { snapshot: false, indicator: false };
        }
        this.settings = next;
        const shown = this.refresh();
        (_a = this.onSettingsChanged) === null || _a === void 0 ? void 0 : _a.call(this, next);
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
        __classPrivateFieldSet(this, _PopupModel_status, "starting", "f");
        __classPrivateFieldSet(this, _PopupModel_startProblem, null, "f");
        return this.refresh();
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
_PopupModel_stops = new WeakMap(), _PopupModel_clock = new WeakMap(), _PopupModel_status = new WeakMap(), _PopupModel_shown = new WeakMap(), _PopupModel_lastKnown = new WeakMap(), _PopupModel_startProblem = new WeakMap(), _PopupModel_snapshotKey = new WeakMap(), _PopupModel_indicatorKey = new WeakMap(), _PopupModel_instances = new WeakSet(), _PopupModel_shownKey = function _PopupModel_shownKey() {
    return JSON.stringify([this.snapshot, this.resources, this.banner]);
}, _PopupModel_send = function _PopupModel_send(lease, kind) {
    var _a;
    const stopper = this.stopper;
    if (stopper === null) {
        return null;
    }
    if (kind === "kill") {
        (_a = this.onKillSent) === null || _a === void 0 ? void 0 : _a.call(this, lease);
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
