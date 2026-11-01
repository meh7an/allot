/** @import { ActiveJob, QueuedJob, UiState } from "../contract.g.mjs" */
/** @import { StopResult } from "./stop-result.js" */

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
  #now;
  /** @type {Map<number, Stop>} */
  #stops = new Map();
  /** @type {UiState | null} */
  #seen = null;

  /** @param {() => number} now The local clock in milliseconds; the tests pass their own. */
  constructor(now) {
    this.#now = now;
  }

  /**
   * The cards with a stop, as they read now; call `expire` first.
   *
   * @returns {Map<number, StopView>}
   */
  get views() {
    /** @type {Map<number, StopView>} */
    const views = new Map();
    this.#stops.forEach((stop, lease) => views.set(lease, this.#view(stop)));
    return views;
  }

  /**
   * Whether a Kill? confirm is up, which Esc cancels before it closes the popup.
   *
   * @returns {boolean}
   */
  get isConfirming() {
    return [...this.#stops.values()].some((stop) => stop.phase === "confirming");
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
    for (const stop of this.#stops.values()) {
      const held = this.#age(stop);
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
    const before = this.#seen;
    this.#seen = state;
    for (const [lease, stop] of [...this.#stops]) {
      const held = find(state, lease);
      if (held !== null && heldKind(held) === stop.kind) {
        stop.held = held;
      } else if (stop.phase === "confirming" || stop.phase === "failed") {
        this.#stops.delete(lease);
      }
    }
    if (before !== null) {
      this.#holdOutsideEnds(before, state);
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
    const stop = this.#stops.get(lease);
    if (stop !== undefined) {
      switch (stop.phase) {
        case "confirming":
          if (this.#age(stop) <= DOUBLE_PRESS_MS) {
            return "ignored";
          }
          stop.phase = "sending";
          stop.since = this.#now();
          return "send";
        case "sending":
        case "ended":
          return "ignored";
        case "failed":
          break;
      }
    }
    const held = find(this.#seen, lease);
    if (held === null || held.active === null) {
      return "ignored";
    }
    this.#stops.set(lease, newStop("kill", "confirming", this.#now(), held));
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
    const stop = this.#stops.get(lease);
    if (stop !== undefined && stop.phase !== "failed") {
      return false;
    }
    const held = find(this.#seen, lease);
    if (held === null || held.queued === null) {
      return false;
    }
    this.#stops.set(lease, newStop("remove", "sending", this.#now(), held));
    return true;
  }

  /**
   * Esc on Kill?.
   *
   * @param {number} lease
   * @returns {boolean} True when a confirm was cancelled.
   */
  cancel(lease) {
    if (this.#stops.get(lease)?.phase !== "confirming") {
      return false;
    }
    this.#stops.delete(lease);
    return true;
  }

  /**
   * Cancels every confirm: the popup closed, or the service went away.
   *
   * @returns {boolean} True when any was cancelled.
   */
  cancelConfirms() {
    return this.#drop((stop) => stop.phase === "confirming");
  }

  /**
   * allotd's answer to a request `pressKill` or `pressRemove` sent.
   *
   * @param {number} lease
   * @param {StopResult} result
   */
  finished(lease, result) {
    const stop = this.#stops.get(lease);
    if (stop === undefined || stop.phase !== "sending") {
      return;
    }
    stop.phase = result.ended ? "ended" : "failed";
    stop.since = this.#now();
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
    return this.#drop((stop) => {
      const age = this.#age(stop);
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
    const missing = [...this.#stops.values()]
      .filter((stop) => stop.phase === "sending" || stop.phase === "ended")
      .filter((stop) => find(state, stop.held.leaseId) === null)
      .sort((a, b) => a.held.index - b.held.index);
    for (const { held } of missing) {
      if (held.active !== null) {
        active.splice(Math.min(held.index, active.length), 0, held.active);
      } else if (held.queued !== null) {
        queue.splice(Math.min(held.index, queue.length), 0, held.queued);
      }
    }
    return { active, queue };
  }

  /**
   * A running job allotd stopped listing without a stop of this popup holds its card too, read
   * from allotd's ended list: released (Finished), killed elsewhere, or ended by allotd.
   * Removals, queued requests and benchmarks (their Finished card comes from recent runs) are
   * not held, and neither is anything the ended list does not explain.
   *
   * @param {UiState} before
   * @param {UiState} state
   */
  #holdOutsideEnds(before, state) {
    before.active.forEach((job, index) => {
      const { lease } = job;
      if (
        this.#stops.has(lease.id) ||
        lease.kind === "benchmark_exclusive" ||
        find(state, lease.id) !== null
      ) {
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
      } else if (ended.cause === "killed") {
        kind = "killed-outside";
      }
      const held = { leaseId: lease.id, active: job, queued: null, index };
      const stop = newStop(kind, "ended", this.#now(), held);
      stop.message = kind === "ended-by-allotd" ? ended.reason : "";
      stop.endedAtMs = ended.ended_at_ms;
      this.#stops.set(lease.id, stop);
    });
  }

  /**
   * @param {(stop: Stop) => boolean} predicate
   * @returns {boolean} True when any was dropped.
   */
  #drop(predicate) {
    let dropped = false;
    for (const [lease, stop] of [...this.#stops]) {
      if (predicate(stop)) {
        this.#stops.delete(lease);
        dropped = true;
      }
    }
    return dropped;
  }

  /**
   * @param {Stop} stop
   * @returns {number}
   */
  #age(stop) {
    return this.#now() - stop.since;
  }

  /**
   * @param {Stop} stop
   * @returns {StopView}
   */
  #view(stop) {
    const elapsed = Math.floor(this.#age(stop) / SECOND_MS);
    const left =
      stop.phase === "confirming" ? Math.max(1, CONFIRM_FOR_MS / SECOND_MS - elapsed) : 0;
    return {
      kind: stop.kind,
      phase: stop.phase,
      secondsLeft: left,
      message: stop.message,
      endedAtMs: stop.endedAtMs,
    };
  }
}

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
  if (state === null) {
    return null;
  }
  const activeIndex = state.active.findIndex((job) => job.lease.id === lease);
  if (activeIndex >= 0) {
    const active = state.active[activeIndex] ?? null;
    return { leaseId: lease, active, queued: null, index: activeIndex };
  }
  const queuedIndex = state.queue.findIndex((job) => job.lease.id === lease);
  if (queuedIndex >= 0) {
    const queued = state.queue[queuedIndex] ?? null;
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
