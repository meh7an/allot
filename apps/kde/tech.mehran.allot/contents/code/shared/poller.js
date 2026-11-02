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
 * Starts allotd for Start Service.
 *
 * @typedef {object} ServiceLauncher
 * @property {() => Promise<string | null>} start
 *   Starts allotd without waiting for it to answer; resolves with why not, or `null`.
 */
/** How long Start Service reports `starting` before giving up. */
export const START_TIMEOUT_MS = 10000;
/**
 * Polls the daemon on the `PollSchedule` and keeps the last state it served, which the
 * service-down view shows dimmed. Start Service launches allotd and reports it as starting until
 * it answers or `START_TIMEOUT_MS` passes.
 */
export class DaemonPoller {
    /**
     * @param {DaemonClient} client
     * @param {Timers} timers
     * @param {object} [options]
     * @param {ServiceLauncher | null} [options.launcher] Starts allotd for Start Service; without
     *   one, Start Service only polls.
     * @param {() => number} [options.now] The local clock of the starting window, in milliseconds.
     */
    constructor(client, timers, options = {}) {
        var _a, _b;
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
        /**
         * Why the last Start Service failed: the launch itself, or allotd not answering in time.
         * Cleared by the next start and once allotd answers.
         *
         * @type {string | null}
         */
        this.startProblem = null;
        /** @type {number | null} */
        this._startingUntil = null;
        this._schedule = new PollSchedule();
        this._popupOpen = false;
        this._looping = false;
        this._stopped = false;
        this._wakePending = false;
        /** @type {unknown} */
        this._timer = null;
        /** @type {(() => void) | null} */
        this._wake = null;
        this._client = client;
        this._timers = timers;
        this._launcher = (_a = options.launcher) !== null && _a !== void 0 ? _a : null;
        this._now = (_b = options.now) !== null && _b !== void 0 ? _b : (() => Date.now());
    }
    /**
     * Launches allotd and polls quickly until it answers or `START_TIMEOUT_MS` passes.
     *
     * @returns {Promise<string | null>} `null` when allotd was launched, else why not.
     */
    startService() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a;
            var _b;
            const problem = (_b = (yield (_a = this._launcher) === null || _a === void 0 ? void 0 : _a.start())) !== null && _b !== void 0 ? _b : null;
            // A failed launch polls too, so whoever shows the poll shows the problem at once.
            this.startProblem = problem;
            if (problem === null) {
                this._startingUntil = this._now() + START_TIMEOUT_MS;
            }
            this.pollNow();
            return problem;
        });
    }
    /**
     * Asks allotd to stop, then polls at once so the icon and popup show it stopped.
     *
     * @returns {Promise<string | null>} `null` when allotd accepted, else why not.
     */
    stopService() {
        return __awaiter(this, void 0, void 0, function* () {
            const problem = yield this._client.shutdown();
            this.pollNow();
            return problem;
        });
    }
    /**
     * Switches between the open and closed cadence; opening also polls at once.
     *
     * @param {boolean} open
     */
    setPopupOpen(open) {
        this._popupOpen = open;
        if (open) {
            this.pollNow();
        }
    }
    /**
     * Polls at once and restarts the backoff. A wake that arrives mid-poll ends the next wait
     * instead of being lost.
     */
    pollNow() {
        this._schedule.reset();
        if (this._wake === null) {
            this._wakePending = true;
        }
        else {
            this._endWait();
        }
    }
    /** Polls until `stop`. */
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            if (this._looping) {
                return;
            }
            this._looping = true;
            this._stopped = false;
            while (!this._stopped) {
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
                yield this._wait(delay);
            }
            this._looping = false;
        });
    }
    /**
     * Ends the loop and drops a poll in flight; the caller cancels its request, since the
     * transport owns it.
     */
    stop() {
        this._stopped = true;
        this._endWait();
    }
    /**
     * @returns {Promise<number>} Milliseconds to wait before the next poll.
     */
    pollOnce() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a;
            const polled = yield this._client.poll();
            if (this._stopped) {
                throw new CancelledError();
            }
            const result = this._whileStarting(polled);
            const delay = this._schedule.next(this._popupOpen, result.status);
            if (result.state !== null) {
                this.lastKnown = result.state;
            }
            this.latest = result;
            (_a = this.onUpdate) === null || _a === void 0 ? void 0 : _a.call(this, result);
            return delay;
        });
    }
    /**
     * A launched allotd takes a moment to bind its port; until then it looks stopped.
     *
     * @param {PollResult} result
     * @returns {PollResult}
     */
    _whileStarting(result) {
        if (result.status !== "stopped") {
            this._startingUntil = null;
            this.startProblem = null;
            return result;
        }
        if (this._startingUntil === null) {
            return result;
        }
        if (this._now() < this._startingUntil) {
            return Object.assign(Object.assign({}, result), { status: "starting" });
        }
        this._startingUntil = null;
        const detail = `allotd did not answer within ${START_TIMEOUT_MS / 1000} s; ` + "see logs/allotd.log";
        this.startProblem = detail;
        return Object.assign(Object.assign({}, result), { status: "stopped", detail });
    }
    /**
     * @param {number} ms
     * @returns {Promise<void>}
     */
    _wait(ms) {
        if (this._wakePending || this._stopped) {
            this._wakePending = false;
            return Promise.resolve();
        }
        return new Promise((resolve) => {
            this._wake = resolve;
            this._timer = this._timers.setTimeout(() => {
                this._timer = null;
                this._endWait();
            }, ms);
        });
    }
    _endWait() {
        if (this._timer !== null) {
            this._timers.clearTimeout(this._timer);
            this._timer = null;
        }
        const wake = this._wake;
        this._wake = null;
        wake === null || wake === void 0 ? void 0 : wake();
    }
}
