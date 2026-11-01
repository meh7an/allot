// Generated from apps/gnome/allot@mehran.tech/lib/notification-rules.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
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
var _NotificationRules_instances, _NotificationRules_now, _NotificationRules_long, _NotificationRules_runs, _NotificationRules_notices, _NotificationRules_localKills, _NotificationRules_merged, _NotificationRules_mergeStart, _NotificationRules_mergeWindows, _NotificationRules_isBaselined, _NotificationRules_wasRunning, _NotificationRules_stoppedPolls, _NotificationRules_isStopShown, _NotificationRules_stopChosenAt, _NotificationRules_observeService, _NotificationRules_baseline, _NotificationRules_observeJobs, _NotificationRules_finished;
import * as text from "./notification-text.js";
/** @import { ActiveJob, RecentRun, UiSession, UiState } from "../contract.g.mjs" */
/** @import { ServiceStatus } from "./client.js" */
/** @import { NotificationMessage } from "./notification-text.js" */
/** @import { NotificationToggles } from "./settings.js" */
/**
 * What one poll changes: notifications to show, and tags of shown ones that no longer apply.
 *
 * @typedef {object} NotificationUpdate
 * @property {NotificationMessage[]} show
 *   New notifications, oldest event first.
 * @property {string[]} withdraw
 */
/**
 * @typedef {object} ObservedPoll
 * @property {ServiceStatus} status
 * @property {UiState | null} live
 *   The state allotd served, or `null` while it is not running.
 * @property {UiState | null} lastKnown
 *   The newest state allotd served, for the session count after a stop.
 * @property {NotificationToggles} toggles
 * @property {boolean} popupOpen
 *   The popup is showing, so the user sees events already.
 */
/** A job is running long at this multiple of its estimate. */
export const LONG_FACTOR = 1.5;
/** A service stop seen on a single poll is a restart (`allot install`, for one), not a stop. */
export const STOPPED_POLLS = 2;
/** Finishes within this time of a window's first notification join it. */
export const MERGE_WINDOW_MS = 60000;
/** How long after the user chose Stop Service a stop counts as theirs. */
export const EXPECTED_STOP_WINDOW_MS = 15000;
/** @type {NotificationToggles} */
const SILENT = {
    service_stopped: false,
    running_long: false,
    benchmark_finished: false,
    job_finished: false,
    killed_outside_flyout: false,
};
/**
 * Decides from each poll which notifications to show (handoff section 9), as the Windows tray
 * app's `ToastRules`. Everything is kept in memory: the first live state after the extension
 * starts is a baseline whose jobs, runs and notices never notify, so a restart repeats nothing,
 * and an event from while it was not running is not shown. An event that comes while its toggle
 * is off or the popup is open counts as seen without a notification.
 */
export class NotificationRules {
    /** @param {() => number} [now] The local clock in milliseconds; the tests pass their own. */
    constructor(now = () => Date.now()) {
        _NotificationRules_instances.add(this);
        _NotificationRules_now.set(this, void 0);
        /** @type {Set<number>} */
        _NotificationRules_long.set(this, new Set());
        /** @type {Set<string>} */
        _NotificationRules_runs.set(this, new Set());
        /** @type {Set<number>} */
        _NotificationRules_notices.set(this, new Set());
        /** @type {Set<number>} */
        _NotificationRules_localKills.set(this, new Set());
        /** @type {RecentRun[]} */
        _NotificationRules_merged.set(this, []);
        /** @type {number | null} */
        _NotificationRules_mergeStart.set(this, null);
        _NotificationRules_mergeWindows.set(this, 0);
        _NotificationRules_isBaselined.set(this, false);
        _NotificationRules_wasRunning.set(this, false);
        _NotificationRules_stoppedPolls.set(this, 0);
        _NotificationRules_isStopShown.set(this, false);
        /** @type {number | null} */
        _NotificationRules_stopChosenAt.set(this, null);
        __classPrivateFieldSet(this, _NotificationRules_now, now, "f");
    }
    /**
     * The popup is about to kill this lease, so its notice is not a kill from outside. Called
     * before the request, so a poll that sees the notice first cannot race it.
     *
     * @param {number} lease
     */
    noteLocalKill(lease) {
        __classPrivateFieldGet(this, _NotificationRules_localKills, "f").add(lease);
    }
    /** The user chose Stop Service, so the stop that follows does not notify. */
    expectStop() {
        __classPrivateFieldSet(this, _NotificationRules_stopChosenAt, __classPrivateFieldGet(this, _NotificationRules_now, "f").call(this), "f");
    }
    /**
     * @param {ObservedPoll} poll
     * @returns {NotificationUpdate}
     */
    observe(poll) {
        const allowed = poll.popupOpen ? SILENT : poll.toggles;
        /** @type {NotificationUpdate} */
        const update = { show: [], withdraw: [] };
        __classPrivateFieldGet(this, _NotificationRules_instances, "m", _NotificationRules_observeService).call(this, poll, allowed, update);
        if (poll.live !== null) {
            if (__classPrivateFieldGet(this, _NotificationRules_isBaselined, "f")) {
                __classPrivateFieldGet(this, _NotificationRules_instances, "m", _NotificationRules_observeJobs).call(this, poll.live, allowed, update.show);
            }
            else {
                __classPrivateFieldGet(this, _NotificationRules_instances, "m", _NotificationRules_baseline).call(this, poll.live);
            }
        }
        return update;
    }
}
_NotificationRules_now = new WeakMap(), _NotificationRules_long = new WeakMap(), _NotificationRules_runs = new WeakMap(), _NotificationRules_notices = new WeakMap(), _NotificationRules_localKills = new WeakMap(), _NotificationRules_merged = new WeakMap(), _NotificationRules_mergeStart = new WeakMap(), _NotificationRules_mergeWindows = new WeakMap(), _NotificationRules_isBaselined = new WeakMap(), _NotificationRules_wasRunning = new WeakMap(), _NotificationRules_stoppedPolls = new WeakMap(), _NotificationRules_isStopShown = new WeakMap(), _NotificationRules_stopChosenAt = new WeakMap(), _NotificationRules_instances = new WeakSet(), _NotificationRules_observeService = function _NotificationRules_observeService(poll, allowed, update) {
    var _a;
    var _b;
    if (poll.status === "running") {
        __classPrivateFieldSet(this, _NotificationRules_wasRunning, true, "f");
        __classPrivateFieldSet(this, _NotificationRules_stoppedPolls, 0, "f");
        if (__classPrivateFieldGet(this, _NotificationRules_isStopShown, "f")) {
            __classPrivateFieldSet(this, _NotificationRules_isStopShown, false, "f");
            update.withdraw.push(text.SERVICE_TAG);
        }
        return;
    }
    // Starting and Incompatible change nothing: neither is a running service going away.
    if (poll.status !== "stopped" || !__classPrivateFieldGet(this, _NotificationRules_wasRunning, "f")) {
        return;
    }
    __classPrivateFieldSet(this, _NotificationRules_stoppedPolls, __classPrivateFieldGet(this, _NotificationRules_stoppedPolls, "f") + 1, "f");
    if (__classPrivateFieldGet(this, _NotificationRules_stoppedPolls, "f") < STOPPED_POLLS) {
        return;
    }
    __classPrivateFieldSet(this, _NotificationRules_wasRunning, false, "f");
    const chosenAt = __classPrivateFieldGet(this, _NotificationRules_stopChosenAt, "f");
    __classPrivateFieldSet(this, _NotificationRules_stopChosenAt, null, "f");
    const chosen = chosenAt !== null && __classPrivateFieldGet(this, _NotificationRules_now, "f").call(this) - chosenAt <= EXPECTED_STOP_WINDOW_MS;
    if (!chosen && allowed.service_stopped) {
        update.show.push(text.serviceStopped((_b = (_a = poll.lastKnown) === null || _a === void 0 ? void 0 : _a.sessions.length) !== null && _b !== void 0 ? _b : 0));
        __classPrivateFieldSet(this, _NotificationRules_isStopShown, true, "f");
    }
}, _NotificationRules_baseline = function _NotificationRules_baseline(live) {
    __classPrivateFieldSet(this, _NotificationRules_isBaselined, true, "f");
    for (const job of live.active) {
        if (isLong(job, live.generated_at_ms)) {
            __classPrivateFieldGet(this, _NotificationRules_long, "f").add(job.lease.id);
        }
    }
    for (const run of live.recent) {
        __classPrivateFieldGet(this, _NotificationRules_runs, "f").add(runKey(run));
    }
    for (const notice of live.notices) {
        __classPrivateFieldGet(this, _NotificationRules_notices, "f").add(notice.lease_id);
    }
}, _NotificationRules_observeJobs = function _NotificationRules_observeJobs(live, allowed, show) {
    var _a;
    /** @type {Map<string, UiSession>} */
    const sessions = new Map();
    for (const session of live.sessions) {
        if (!sessions.has(session.session)) {
            sessions.set(session.session, session);
        }
    }
    keepOnly(__classPrivateFieldGet(this, _NotificationRules_long, "f"), live.active.map((job) => job.lease.id));
    for (const job of live.active) {
        if (isLong(job, live.generated_at_ms) && !__classPrivateFieldGet(this, _NotificationRules_long, "f").has(job.lease.id)) {
            __classPrivateFieldGet(this, _NotificationRules_long, "f").add(job.lease.id);
            if (allowed.running_long) {
                const session = (_a = sessions.get(job.lease.holder.session)) !== null && _a !== void 0 ? _a : null;
                show.push(text.runningLong(job, session, live.generated_at_ms));
            }
        }
    }
    /** @type {RecentRun[]} */
    const finished = [];
    for (const run of [...live.recent].reverse()) {
        const key = runKey(run);
        const isNew = !__classPrivateFieldGet(this, _NotificationRules_runs, "f").has(key);
        __classPrivateFieldGet(this, _NotificationRules_runs, "f").add(key);
        // A killed run has the kill's notification or the popup's own Stopped card.
        if (!isNew || (run.end_cause !== null && run.end_cause !== "released")) {
            continue;
        }
        if (run.noise !== null) {
            if (allowed.benchmark_finished && run.lease_id !== null) {
                show.push(text.benchmarkFinished(run, run.noise));
            }
        }
        else if (allowed.job_finished) {
            finished.push(run);
        }
    }
    keepOnly(__classPrivateFieldGet(this, _NotificationRules_runs, "f"), live.recent.map(runKey));
    if (finished.length > 0) {
        show.push(__classPrivateFieldGet(this, _NotificationRules_instances, "m", _NotificationRules_finished).call(this, finished, sessions));
    }
    for (const notice of [...live.notices].reverse()) {
        const isNew = !__classPrivateFieldGet(this, _NotificationRules_notices, "f").has(notice.lease_id);
        __classPrivateFieldGet(this, _NotificationRules_notices, "f").add(notice.lease_id);
        if (isNew &&
            notice.cause === "killed" &&
            !__classPrivateFieldGet(this, _NotificationRules_localKills, "f").has(notice.lease_id) &&
            allowed.killed_outside_flyout) {
            show.push(text.killedOutside(notice));
        }
    }
    keepOnly(__classPrivateFieldGet(this, _NotificationRules_notices, "f"), live.notices.map((notice) => notice.lease_id));
}, _NotificationRules_finished = function _NotificationRules_finished(runs, sessions) {
    var _a;
    const now = __classPrivateFieldGet(this, _NotificationRules_now, "f").call(this);
    if (__classPrivateFieldGet(this, _NotificationRules_merged, "f").length === 0 ||
        __classPrivateFieldGet(this, _NotificationRules_mergeStart, "f") === null ||
        now - __classPrivateFieldGet(this, _NotificationRules_mergeStart, "f") >= MERGE_WINDOW_MS) {
        __classPrivateFieldSet(this, _NotificationRules_merged, [], "f");
        __classPrivateFieldSet(this, _NotificationRules_mergeStart, now, "f");
        __classPrivateFieldSet(this, _NotificationRules_mergeWindows, __classPrivateFieldGet(this, _NotificationRules_mergeWindows, "f") + 1, "f");
    }
    __classPrivateFieldGet(this, _NotificationRules_merged, "f").push(...runs);
    const tag = `done-${__classPrivateFieldGet(this, _NotificationRules_mergeWindows, "f")}`;
    const [only] = __classPrivateFieldGet(this, _NotificationRules_merged, "f");
    if (__classPrivateFieldGet(this, _NotificationRules_merged, "f").length > 1 || only === undefined) {
        return text.jobsFinished(__classPrivateFieldGet(this, _NotificationRules_merged, "f"), tag);
    }
    const session = only.session === null ? null : ((_a = sessions.get(only.session)) !== null && _a !== void 0 ? _a : null);
    return text.jobFinished(only, session, tag);
};
/**
 * @param {ActiveJob} job
 * @param {number} nowMs
 * @returns {boolean}
 */
function isLong(job, nowMs) {
    var _a;
    const { lease } = job;
    const elapsedMs = nowMs - ((_a = lease.granted_at_ms) !== null && _a !== void 0 ? _a : lease.requested_at_ms);
    const { est_secs: estSecs } = lease.profile;
    return estSecs > 0 && elapsedMs >= LONG_FACTOR * estSecs * 1000;
}
/**
 * Runs from before v0.2 have no lease; their times tell them apart.
 *
 * @param {RecentRun} run
 * @returns {string}
 */
function runKey(run) {
    return `${run.lease_id}/${run.started_at_ms}/${run.ended_at_ms}`;
}
/**
 * @template T
 * @param {Set<T>} set
 * @param {T[]} wanted
 */
function keepOnly(set, wanted) {
    const keep = new Set(wanted);
    for (const item of [...set]) {
        if (!keep.has(item)) {
            set.delete(item);
        }
    }
}
