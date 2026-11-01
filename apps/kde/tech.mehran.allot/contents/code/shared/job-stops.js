// Generated from apps/gnome/allot@mehran.tech/lib/job-stops.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/** @import { ActiveJob, QueuedJob, UiState } from "../contract.g.mjs" */
/** @import { StopResult } from "./stop-result.js" */
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
var _JobStops_instances, _JobStops_now, _JobStops_stops, _JobStops_seen, _JobStops_holdOutsideEnds, _JobStops_drop, _JobStops_age, _JobStops_view;
/**
 * - `kill`: kill a running job's process tree, after the two-step confirm.
 * - `remove`: remove a queued request, without a confirm because no work is lost.
 * - `finished`: not a stop; the job's session released it, and its card holds as Finished.
 * - `killed-outside`: not a stop of this popup; `allot kill` or another Allot app killed it.
 * - `ended-by-allotd`: not a stop; allotd ended the lease (idle session, quick-lease overrun,
 *   holder gone).
 *
 * @typedef {"kill" | "remove" | "finished" | "killed-outside" | "ended-by-allotd"} StopKind
 */
/**
 * - `confirming`: the red Kill? button is up and counting down.
 * - `sending`: the request is on its way to allotd.
 * - `ended`: the lease ended; the card holds Stopped, Removed or how it ended, then collapses.
 * - `failed`: allotd refused or did not answer; the card says why in red.
 *
 * @typedef {"confirming" | "sending" | "ended" | "failed"} StopPhase
 */
/**
 * What this popup is doing to one job, as its card shows it (the prototype's per-job
 * `killPhase` and `removePhase`).
 *
 * @typedef {object} StopView
 * @property {StopKind} kind
 * @property {StopPhase} phase
 * @property {number} secondsLeft
 *   While confirming, the countdown from 4 to 1; 0 otherwise.
 * @property {string} message
 *   Ended: a note for the Stopped line, or empty; allotd's reason for `ended-by-allotd`.
 *   Failed: why not.
 * @property {number | null} endedAtMs
 *   Ended: when allotd ended the lease, in the daemon's clock.
 */
/**
 * - `ignored`: nothing happens: the double-press guard, a stop already under way, or an
 *   unknown job.
 * - `confirming`: the first press of Kill; the card asks Kill?.
 * - `send`: the caller sends the request to allotd and reports back with `finished`.
 *
 * @typedef {"ignored" | "confirming" | "send"} StopPress
 */
/**
 * The jobs a snapshot shows: allotd's lists, with the cards of ended stops put back.
 *
 * @typedef {object} ShownJobs
 * @property {ActiveJob[]} active
 * @property {QueuedJob[]} queue
 */
/**
 * The job as last seen: the running job or the waiting request, and its place in allotd's
 * active or queue list.
 *
 * @typedef {object} Held
 * @property {number} leaseId
 * @property {ActiveJob | null} active
 * @property {QueuedJob | null} queued
 * @property {number} index
 */
/**
 * @typedef {object} Stop
 * @property {StopKind} kind
 * @property {StopPhase} phase
 * @property {number} since
 *   When it entered the phase, in local milliseconds.
 * @property {Held} held
 * @property {string} message
 * @property {number | null} endedAtMs
 */
/** How long Kill? waits for the second press, in milliseconds. */
export const CONFIRM_FOR_MS = 4000;
/** A second press this soon after the first is ignored, so a double-click can't kill. */
export const DOUBLE_PRESS_MS = 300;
/** How long Stopped or Removed shows before the card collapses. */
export const ENDED_FOR_MS = 4000;
/** How long a refusal shows before the card reads normally again. */
export const FAILED_FOR_MS = 6000;
const SECOND_MS = 1000;
/**
 * The Kill and Remove state of every card (handoff section 4): the two-step Kill? confirm, the
 * request in flight, and the Stopped, Removed or refused line that holds before the card goes.
 * It keeps the last seen copy of a job whose stop is under way or done, because allotd stops
 * listing the lease the moment it ends, while the card still holds for `ENDED_FOR_MS`.
 */
export class JobStops {
    /** @param {() => number} now The local clock in milliseconds; the tests pass their own. */
    constructor(now) {
        _JobStops_instances.add(this);
        _JobStops_now.set(this, void 0);
        /** @type {Map<number, Stop>} */
        _JobStops_stops.set(this, new Map());
        /** @type {UiState | null} */
        _JobStops_seen.set(this, null);
        __classPrivateFieldSet(this, _JobStops_now, now, "f");
    }
    /**
     * The cards with a stop, as they read now; call `expire` first.
     *
     * @returns {Map<number, StopView>}
     */
    get views() {
        /** @type {Map<number, StopView>} */
        const views = new Map();
        __classPrivateFieldGet(this, _JobStops_stops, "f").forEach((stop, lease) => views.set(lease, __classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_view).call(this, stop)));
        return views;
    }
    /**
     * Whether a Kill? confirm is up, which Esc cancels before it closes the popup.
     *
     * @returns {boolean}
     */
    get isConfirming() {
        return [...__classPrivateFieldGet(this, _JobStops_stops, "f").values()].some((stop) => stop.phase === "confirming");
    }
    /**
     * The milliseconds until a card's text next changes by itself (a countdown second, or a hold
     * running out), or `null` when none will.
     *
     * @returns {number | null}
     */
    get nextChangeMs() {
        /** @type {number | null} */
        let next = null;
        for (const stop of __classPrivateFieldGet(this, _JobStops_stops, "f").values()) {
            const held = __classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_age).call(this, stop);
            /** @type {number | null} */
            let due = null;
            switch (stop.phase) {
                case "confirming":
                    due = SECOND_MS - (held % SECOND_MS);
                    break;
                case "ended":
                    due = ENDED_FOR_MS - held;
                    break;
                case "failed":
                    due = FAILED_FOR_MS - held;
                    break;
                case "sending":
                    break;
            }
            if (due !== null && (next === null || due < next)) {
                next = Math.max(due, 0);
            }
        }
        return next;
    }
    /**
     * Takes a poll's live state: copies of the jobs with a stop are refreshed, and a confirm or
     * refusal is dropped once its job is gone or has moved between running and waiting.
     *
     * @param {UiState} state
     */
    observe(state) {
        const before = __classPrivateFieldGet(this, _JobStops_seen, "f");
        __classPrivateFieldSet(this, _JobStops_seen, state, "f");
        for (const [lease, stop] of [...__classPrivateFieldGet(this, _JobStops_stops, "f")]) {
            const held = find(state, lease);
            if (held !== null && heldKind(held) === stop.kind) {
                stop.held = held;
            }
            else if (stop.phase === "confirming" || stop.phase === "failed") {
                __classPrivateFieldGet(this, _JobStops_stops, "f").delete(lease);
            }
        }
        if (before !== null) {
            __classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_holdOutsideEnds).call(this, before, state);
        }
    }
    /**
     * A press of a running job's Kill, or of its Kill? confirm.
     *
     * @param {number} lease
     * @returns {StopPress}
     */
    pressKill(lease) {
        this.expire();
        const stop = __classPrivateFieldGet(this, _JobStops_stops, "f").get(lease);
        if (stop !== undefined) {
            switch (stop.phase) {
                case "confirming":
                    if (__classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_age).call(this, stop) <= DOUBLE_PRESS_MS) {
                        return "ignored";
                    }
                    stop.phase = "sending";
                    stop.since = __classPrivateFieldGet(this, _JobStops_now, "f").call(this);
                    return "send";
                case "sending":
                case "ended":
                    return "ignored";
                case "failed":
                    break;
            }
        }
        const held = find(__classPrivateFieldGet(this, _JobStops_seen, "f"), lease);
        if (held === null || held.active === null) {
            return "ignored";
        }
        __classPrivateFieldGet(this, _JobStops_stops, "f").set(lease, newStop("kill", "confirming", __classPrivateFieldGet(this, _JobStops_now, "f").call(this), held));
        return "confirming";
    }
    /**
     * A press of a queued request's Remove.
     *
     * @param {number} lease
     * @returns {boolean} True when the caller sends the request to allotd.
     */
    pressRemove(lease) {
        this.expire();
        const stop = __classPrivateFieldGet(this, _JobStops_stops, "f").get(lease);
        if (stop !== undefined && stop.phase !== "failed") {
            return false;
        }
        const held = find(__classPrivateFieldGet(this, _JobStops_seen, "f"), lease);
        if (held === null || held.queued === null) {
            return false;
        }
        __classPrivateFieldGet(this, _JobStops_stops, "f").set(lease, newStop("remove", "sending", __classPrivateFieldGet(this, _JobStops_now, "f").call(this), held));
        return true;
    }
    /**
     * Esc on Kill?.
     *
     * @param {number} lease
     * @returns {boolean} True when a confirm was cancelled.
     */
    cancel(lease) {
        var _a;
        if (((_a = __classPrivateFieldGet(this, _JobStops_stops, "f").get(lease)) === null || _a === void 0 ? void 0 : _a.phase) !== "confirming") {
            return false;
        }
        __classPrivateFieldGet(this, _JobStops_stops, "f").delete(lease);
        return true;
    }
    /**
     * Cancels every confirm: the popup closed, or the service went away.
     *
     * @returns {boolean} True when any was cancelled.
     */
    cancelConfirms() {
        return __classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_drop).call(this, (stop) => stop.phase === "confirming");
    }
    /**
     * allotd's answer to a request `pressKill` or `pressRemove` sent.
     *
     * @param {number} lease
     * @param {StopResult} result
     */
    finished(lease, result) {
        const stop = __classPrivateFieldGet(this, _JobStops_stops, "f").get(lease);
        if (stop === undefined || stop.phase !== "sending") {
            return;
        }
        stop.phase = result.ended ? "ended" : "failed";
        stop.since = __classPrivateFieldGet(this, _JobStops_now, "f").call(this);
        stop.message = result.text;
        if (result.ended) {
            stop.endedAtMs = result.endedAtMs;
        }
    }
    /**
     * Drops confirms past their countdown and holds past their time.
     *
     * @returns {boolean} True when any was dropped.
     */
    expire() {
        return __classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_drop).call(this, (stop) => {
            const age = __classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_age).call(this, stop);
            switch (stop.phase) {
                case "confirming":
                    return age >= CONFIRM_FOR_MS;
                case "ended":
                    return age >= ENDED_FOR_MS;
                case "failed":
                    return age >= FAILED_FOR_MS;
                case "sending":
                    return false;
            }
        });
    }
    /**
     * `state`'s jobs with the last seen copy of every job whose stop is under way or done put
     * back where it was, once allotd no longer lists it.
     *
     * @param {UiState} state
     * @returns {ShownJobs}
     */
    merge(state) {
        const active = [...state.active];
        const queue = [...state.queue];
        const missing = [...__classPrivateFieldGet(this, _JobStops_stops, "f").values()]
            .filter((stop) => stop.phase === "sending" || stop.phase === "ended")
            .filter((stop) => find(state, stop.held.leaseId) === null)
            .sort((a, b) => a.held.index - b.held.index);
        for (const { held } of missing) {
            if (held.active !== null) {
                active.splice(Math.min(held.index, active.length), 0, held.active);
            }
            else if (held.queued !== null) {
                queue.splice(Math.min(held.index, queue.length), 0, held.queued);
            }
        }
        return { active, queue };
    }
}
_JobStops_now = new WeakMap(), _JobStops_stops = new WeakMap(), _JobStops_seen = new WeakMap(), _JobStops_instances = new WeakSet(), _JobStops_holdOutsideEnds = function _JobStops_holdOutsideEnds(before, state) {
    before.active.forEach((job, index) => {
        const { lease } = job;
        if (__classPrivateFieldGet(this, _JobStops_stops, "f").has(lease.id) ||
            lease.kind === "benchmark_exclusive" ||
            find(state, lease.id) !== null) {
            return;
        }
        const ended = state.ended.find((end) => end.lease_id === lease.id);
        if (ended === undefined || ended.cause === "removed") {
            return;
        }
        /** @type {StopKind} */
        let kind = "ended-by-allotd";
        if (ended.cause === "released") {
            kind = "finished";
        }
        else if (ended.cause === "killed") {
            kind = "killed-outside";
        }
        const held = { leaseId: lease.id, active: job, queued: null, index };
        const stop = newStop(kind, "ended", __classPrivateFieldGet(this, _JobStops_now, "f").call(this), held);
        stop.message = kind === "ended-by-allotd" ? ended.reason : "";
        stop.endedAtMs = ended.ended_at_ms;
        __classPrivateFieldGet(this, _JobStops_stops, "f").set(lease.id, stop);
    });
}, _JobStops_drop = function _JobStops_drop(predicate) {
    let dropped = false;
    for (const [lease, stop] of [...__classPrivateFieldGet(this, _JobStops_stops, "f")]) {
        if (predicate(stop)) {
            __classPrivateFieldGet(this, _JobStops_stops, "f").delete(lease);
            dropped = true;
        }
    }
    return dropped;
}, _JobStops_age = function _JobStops_age(stop) {
    return __classPrivateFieldGet(this, _JobStops_now, "f").call(this) - stop.since;
}, _JobStops_view = function _JobStops_view(stop) {
    const elapsed = Math.floor(__classPrivateFieldGet(this, _JobStops_instances, "m", _JobStops_age).call(this, stop) / SECOND_MS);
    const left = stop.phase === "confirming" ? Math.max(1, CONFIRM_FOR_MS / SECOND_MS - elapsed) : 0;
    return {
        kind: stop.kind,
        phase: stop.phase,
        secondsLeft: left,
        message: stop.message,
        endedAtMs: stop.endedAtMs,
    };
};
/**
 * @param {StopKind} kind
 * @param {StopPhase} phase
 * @param {number} since
 * @param {Held} held
 * @returns {Stop}
 */
function newStop(kind, phase, since, held) {
    return { kind, phase, since, held, message: "", endedAtMs: null };
}
/**
 * @param {UiState | null} state
 * @param {number} lease
 * @returns {Held | null}
 */
function find(state, lease) {
    var _a, _b;
    if (state === null) {
        return null;
    }
    const activeIndex = state.active.findIndex((job) => job.lease.id === lease);
    if (activeIndex >= 0) {
        const active = (_a = state.active[activeIndex]) !== null && _a !== void 0 ? _a : null;
        return { leaseId: lease, active, queued: null, index: activeIndex };
    }
    const queuedIndex = state.queue.findIndex((job) => job.lease.id === lease);
    if (queuedIndex >= 0) {
        const queued = (_b = state.queue[queuedIndex]) !== null && _b !== void 0 ? _b : null;
        return { leaseId: lease, active: null, queued, index: queuedIndex };
    }
    return null;
}
/**
 * @param {Held} held
 * @returns {StopKind}
 */
function heldKind(held) {
    return held.active === null ? "remove" : "kill";
}
