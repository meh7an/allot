// Generated from apps/gnome/allot@mehran.tech/lib/poller.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
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
var _DaemonPoller_instances, _DaemonPoller_client, _DaemonPoller_timers, _DaemonPoller_schedule, _DaemonPoller_popupOpen, _DaemonPoller_looping, _DaemonPoller_stopped, _DaemonPoller_wakePending, _DaemonPoller_timer, _DaemonPoller_wake, _DaemonPoller_wait, _DaemonPoller_endWait;
import { CancelledError } from "./client.js";
import { PollSchedule } from "./schedule.js";
/** @import { DaemonClient, PollResult } from "./client.js" */
/** @import { UiState } from "../contract.g.mjs" */
/**
 * The timer calls the poller needs: GLib sources in the shell, fakes in the tests.
 *
 * @typedef {object} Timers
 * @property {(callback: () => void, ms: number) => unknown} setTimeout
 * @property {(id: unknown) => void} clearTimeout
 */
/**
 * Polls the daemon on the `PollSchedule` and keeps the last state it served, which the
 * service-down view shows dimmed.
 */
export class DaemonPoller {
    /**
     * @param {DaemonClient} client
     * @param {Timers} timers
     */
    constructor(client, timers) {
        _DaemonPoller_instances.add(this);
        /**
         * Called after every poll.
         *
         * @type {((result: PollResult) => void) | null}
         */
        this.onUpdate = null;
        /** @type {PollResult | null} */
        this.latest = null;
        /**
         * The newest state allotd served, kept while it is down.
         *
         * @type {UiState | null}
         */
        this.lastKnown = null;
        _DaemonPoller_client.set(this, void 0);
        _DaemonPoller_timers.set(this, void 0);
        _DaemonPoller_schedule.set(this, new PollSchedule());
        _DaemonPoller_popupOpen.set(this, false);
        _DaemonPoller_looping.set(this, false);
        _DaemonPoller_stopped.set(this, false);
        _DaemonPoller_wakePending.set(this, false);
        /** @type {unknown} */
        _DaemonPoller_timer.set(this, null);
        /** @type {(() => void) | null} */
        _DaemonPoller_wake.set(this, null);
        __classPrivateFieldSet(this, _DaemonPoller_client, client, "f");
        __classPrivateFieldSet(this, _DaemonPoller_timers, timers, "f");
    }
    /**
     * Switches between the open and closed cadence; opening also polls at once.
     *
     * @param {boolean} open
     */
    setPopupOpen(open) {
        __classPrivateFieldSet(this, _DaemonPoller_popupOpen, open, "f");
        if (open) {
            this.pollNow();
        }
    }
    /**
     * Polls at once and restarts the backoff. A wake that arrives mid-poll ends the next wait
     * instead of being lost.
     */
    pollNow() {
        __classPrivateFieldGet(this, _DaemonPoller_schedule, "f").reset();
        if (__classPrivateFieldGet(this, _DaemonPoller_wake, "f") === null) {
            __classPrivateFieldSet(this, _DaemonPoller_wakePending, true, "f");
        }
        else {
            __classPrivateFieldGet(this, _DaemonPoller_instances, "m", _DaemonPoller_endWait).call(this);
        }
    }
    /** Polls until `stop`. */
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            if (__classPrivateFieldGet(this, _DaemonPoller_looping, "f")) {
                return;
            }
            __classPrivateFieldSet(this, _DaemonPoller_looping, true, "f");
            __classPrivateFieldSet(this, _DaemonPoller_stopped, false, "f");
            while (!__classPrivateFieldGet(this, _DaemonPoller_stopped, "f")) {
                let delay;
                try {
                    delay = yield this.pollOnce();
                }
                catch (error) {
                    if (error instanceof CancelledError) {
                        break;
                    }
                    throw error;
                }
                yield __classPrivateFieldGet(this, _DaemonPoller_instances, "m", _DaemonPoller_wait).call(this, delay);
            }
            __classPrivateFieldSet(this, _DaemonPoller_looping, false, "f");
        });
    }
    /**
     * Ends the loop and drops a poll in flight; the caller cancels its request, since the
     * transport owns it.
     */
    stop() {
        __classPrivateFieldSet(this, _DaemonPoller_stopped, true, "f");
        __classPrivateFieldGet(this, _DaemonPoller_instances, "m", _DaemonPoller_endWait).call(this);
    }
    /**
     * @returns {Promise<number>} Milliseconds to wait before the next poll.
     */
    pollOnce() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a;
            const result = yield __classPrivateFieldGet(this, _DaemonPoller_client, "f").poll();
            if (__classPrivateFieldGet(this, _DaemonPoller_stopped, "f")) {
                throw new CancelledError();
            }
            const delay = __classPrivateFieldGet(this, _DaemonPoller_schedule, "f").next(__classPrivateFieldGet(this, _DaemonPoller_popupOpen, "f"), result.status);
            if (result.state !== null) {
                this.lastKnown = result.state;
            }
            this.latest = result;
            (_a = this.onUpdate) === null || _a === void 0 ? void 0 : _a.call(this, result);
            return delay;
        });
    }
}
_DaemonPoller_client = new WeakMap(), _DaemonPoller_timers = new WeakMap(), _DaemonPoller_schedule = new WeakMap(), _DaemonPoller_popupOpen = new WeakMap(), _DaemonPoller_looping = new WeakMap(), _DaemonPoller_stopped = new WeakMap(), _DaemonPoller_wakePending = new WeakMap(), _DaemonPoller_timer = new WeakMap(), _DaemonPoller_wake = new WeakMap(), _DaemonPoller_instances = new WeakSet(), _DaemonPoller_wait = function _DaemonPoller_wait(ms) {
    if (__classPrivateFieldGet(this, _DaemonPoller_wakePending, "f") || __classPrivateFieldGet(this, _DaemonPoller_stopped, "f")) {
        __classPrivateFieldSet(this, _DaemonPoller_wakePending, false, "f");
        return Promise.resolve();
    }
    return new Promise((resolve) => {
        __classPrivateFieldSet(this, _DaemonPoller_wake, resolve, "f");
        __classPrivateFieldSet(this, _DaemonPoller_timer, __classPrivateFieldGet(this, _DaemonPoller_timers, "f").setTimeout(() => {
            __classPrivateFieldSet(this, _DaemonPoller_timer, null, "f");
            __classPrivateFieldGet(this, _DaemonPoller_instances, "m", _DaemonPoller_endWait).call(this);
        }, ms), "f");
    });
}, _DaemonPoller_endWait = function _DaemonPoller_endWait() {
    if (__classPrivateFieldGet(this, _DaemonPoller_timer, "f") !== null) {
        __classPrivateFieldGet(this, _DaemonPoller_timers, "f").clearTimeout(__classPrivateFieldGet(this, _DaemonPoller_timer, "f"));
        __classPrivateFieldSet(this, _DaemonPoller_timer, null, "f");
    }
    const wake = __classPrivateFieldGet(this, _DaemonPoller_wake, "f");
    __classPrivateFieldSet(this, _DaemonPoller_wake, null, "f");
    wake === null || wake === void 0 ? void 0 : wake();
};
