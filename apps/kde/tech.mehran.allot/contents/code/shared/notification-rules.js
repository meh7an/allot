// Generated from apps/gnome/allot@mehran.tech/lib/notification-rules.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
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
        /** @type {Set<number>} */
        this._long = new Set();
        /** @type {Set<string>} */
        this._runs = new Set();
        /** @type {Set<number>} */
        this._notices = new Set();
        /** @type {Set<number>} */
        this._localKills = new Set();
        /** @type {RecentRun[]} */
        this._merged = [];
        /** @type {number | null} */
        this._mergeStart = null;
        this._mergeWindows = 0;
        this._isBaselined = false;
        this._wasRunning = false;
        this._stoppedPolls = 0;
        this._isStopShown = false;
        /** @type {number | null} */
        this._stopChosenAt = null;
        this._now = now;
    }
    /**
     * The popup is about to kill this lease, so its notice is not a kill from outside. Called
     * before the request, so a poll that sees the notice first cannot race it.
     *
     * @param {number} lease
     */
    noteLocalKill(lease) {
        this._localKills.add(lease);
    }
    /** The user chose Stop Service, so the stop that follows does not notify. */
    expectStop() {
        this._stopChosenAt = this._now();
    }
    /**
     * @param {ObservedPoll} poll
     * @returns {NotificationUpdate}
     */
    observe(poll) {
        const allowed = poll.popupOpen ? SILENT : poll.toggles;
        /** @type {NotificationUpdate} */
        const update = { show: [], withdraw: [] };
        this._observeService(poll, allowed, update);
        if (poll.live !== null) {
            if (this._isBaselined) {
                this._observeJobs(poll.live, allowed, update.show);
            }
            else {
                this._baseline(poll.live);
            }
        }
        return update;
    }
    /**
     * @param {ObservedPoll} poll
     * @param {NotificationToggles} allowed
     * @param {NotificationUpdate} update
     */
    _observeService(poll, allowed, update) {
        var _a;
        var _b;
        if (poll.status === "running") {
            this._wasRunning = true;
            this._stoppedPolls = 0;
            if (this._isStopShown) {
                this._isStopShown = false;
                update.withdraw.push(text.SERVICE_TAG);
            }
            return;
        }
        // Starting and Incompatible change nothing: neither is a running service going away.
        if (poll.status !== "stopped" || !this._wasRunning) {
            return;
        }
        this._stoppedPolls += 1;
        if (this._stoppedPolls < STOPPED_POLLS) {
            return;
        }
        this._wasRunning = false;
        const chosenAt = this._stopChosenAt;
        this._stopChosenAt = null;
        const chosen = chosenAt !== null && this._now() - chosenAt <= EXPECTED_STOP_WINDOW_MS;
        if (!chosen && allowed.service_stopped) {
            update.show.push(text.serviceStopped((_b = (_a = poll.lastKnown) === null || _a === void 0 ? void 0 : _a.sessions.length) !== null && _b !== void 0 ? _b : 0));
            this._isStopShown = true;
        }
    }
    /** @param {UiState} live */
    _baseline(live) {
        this._isBaselined = true;
        for (const job of live.active) {
            if (isLong(job, live.generated_at_ms)) {
                this._long.add(job.lease.id);
            }
        }
        for (const run of live.recent) {
            this._runs.add(runKey(run));
        }
        for (const notice of live.notices) {
            this._notices.add(notice.lease_id);
        }
    }
    /**
     * Jobs, then runs and notices, each oldest first: allotd lists runs and notices newest first.
     *
     * @param {UiState} live
     * @param {NotificationToggles} allowed
     * @param {NotificationMessage[]} show
     */
    _observeJobs(live, allowed, show) {
        var _a;
        /** @type {Map<string, UiSession>} */
        const sessions = new Map();
        for (const session of live.sessions) {
            if (!sessions.has(session.session)) {
                sessions.set(session.session, session);
            }
        }
        keepOnly(this._long, live.active.map((job) => job.lease.id));
        for (const job of live.active) {
            if (isLong(job, live.generated_at_ms) && !this._long.has(job.lease.id)) {
                this._long.add(job.lease.id);
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
            const isNew = !this._runs.has(key);
            this._runs.add(key);
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
        keepOnly(this._runs, live.recent.map(runKey));
        if (finished.length > 0) {
            show.push(this._finished(finished, sessions));
        }
        for (const notice of [...live.notices].reverse()) {
            const isNew = !this._notices.has(notice.lease_id);
            this._notices.add(notice.lease_id);
            if (isNew &&
                notice.cause === "killed" &&
                !this._localKills.has(notice.lease_id) &&
                allowed.killed_outside_flyout) {
                show.push(text.killedOutside(notice));
            }
        }
        keepOnly(this._notices, live.notices.map((notice) => notice.lease_id));
    }
    /**
     * The first finish opens a window; later ones within it replace its notification with the
     * count.
     *
     * @param {RecentRun[]} runs
     * @param {Map<string, UiSession>} sessions
     * @returns {NotificationMessage}
     */
    _finished(runs, sessions) {
        var _a;
        const now = this._now();
        if (this._merged.length === 0 ||
            this._mergeStart === null ||
            now - this._mergeStart >= MERGE_WINDOW_MS) {
            this._merged = [];
            this._mergeStart = now;
            this._mergeWindows += 1;
        }
        this._merged.push(...runs);
        const tag = `done-${this._mergeWindows}`;
        const [only] = this._merged;
        if (this._merged.length > 1 || only === undefined) {
            return text.jobsFinished(this._merged, tag);
        }
        const session = only.session === null ? null : ((_a = sessions.get(only.session)) !== null && _a !== void 0 ? _a : null);
        return text.jobFinished(only, session, tag);
    }
}
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
