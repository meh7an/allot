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
export const MERGE_WINDOW_MS = 60_000;

/** How long after the user chose Stop Service a stop counts as theirs. */
export const EXPECTED_STOP_WINDOW_MS = 15_000;

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
  #now;
  /** @type {Set<number>} */
  #long = new Set();
  /** @type {Set<string>} */
  #runs = new Set();
  /** @type {Set<number>} */
  #notices = new Set();
  /** @type {Set<number>} */
  #localKills = new Set();
  /** @type {RecentRun[]} */
  #merged = [];
  /** @type {number | null} */
  #mergeStart = null;
  #mergeWindows = 0;
  #isBaselined = false;
  #wasRunning = false;
  #stoppedPolls = 0;
  #isStopShown = false;
  /** @type {number | null} */
  #stopChosenAt = null;

  /** @param {() => number} [now] The local clock in milliseconds; the tests pass their own. */
  constructor(now = () => Date.now()) {
    this.#now = now;
  }

  /**
   * The popup is about to kill this lease, so its notice is not a kill from outside. Called
   * before the request, so a poll that sees the notice first cannot race it.
   *
   * @param {number} lease
   */
  noteLocalKill(lease) {
    this.#localKills.add(lease);
  }

  /** The user chose Stop Service, so the stop that follows does not notify. */
  expectStop() {
    this.#stopChosenAt = this.#now();
  }

  /**
   * @param {ObservedPoll} poll
   * @returns {NotificationUpdate}
   */
  observe(poll) {
    const allowed = poll.popupOpen ? SILENT : poll.toggles;
    /** @type {NotificationUpdate} */
    const update = { show: [], withdraw: [] };
    this.#observeService(poll, allowed, update);
    if (poll.live !== null) {
      if (this.#isBaselined) {
        this.#observeJobs(poll.live, allowed, update.show);
      } else {
        this.#baseline(poll.live);
      }
    }
    return update;
  }

  /**
   * @param {ObservedPoll} poll
   * @param {NotificationToggles} allowed
   * @param {NotificationUpdate} update
   */
  #observeService(poll, allowed, update) {
    if (poll.status === "running") {
      this.#wasRunning = true;
      this.#stoppedPolls = 0;
      if (this.#isStopShown) {
        this.#isStopShown = false;
        update.withdraw.push(text.SERVICE_TAG);
      }
      return;
    }

    // Starting and Incompatible change nothing: neither is a running service going away.
    if (poll.status !== "stopped" || !this.#wasRunning) {
      return;
    }
    this.#stoppedPolls += 1;
    if (this.#stoppedPolls < STOPPED_POLLS) {
      return;
    }
    this.#wasRunning = false;
    const chosenAt = this.#stopChosenAt;
    this.#stopChosenAt = null;
    const chosen = chosenAt !== null && this.#now() - chosenAt <= EXPECTED_STOP_WINDOW_MS;
    if (!chosen && allowed.service_stopped) {
      update.show.push(text.serviceStopped(poll.lastKnown?.sessions.length ?? 0));
      this.#isStopShown = true;
    }
  }

  /** @param {UiState} live */
  #baseline(live) {
    this.#isBaselined = true;
    for (const job of live.active) {
      if (isLong(job, live.generated_at_ms)) {
        this.#long.add(job.lease.id);
      }
    }
    for (const run of live.recent) {
      this.#runs.add(runKey(run));
    }
    for (const notice of live.notices) {
      this.#notices.add(notice.lease_id);
    }
  }

  /**
   * Jobs, then runs and notices, each oldest first: allotd lists runs and notices newest first.
   *
   * @param {UiState} live
   * @param {NotificationToggles} allowed
   * @param {NotificationMessage[]} show
   */
  #observeJobs(live, allowed, show) {
    /** @type {Map<string, UiSession>} */
    const sessions = new Map();
    for (const session of live.sessions) {
      if (!sessions.has(session.session)) {
        sessions.set(session.session, session);
      }
    }

    keepOnly(this.#long, live.active.map((job) => job.lease.id));
    for (const job of live.active) {
      if (isLong(job, live.generated_at_ms) && !this.#long.has(job.lease.id)) {
        this.#long.add(job.lease.id);
        if (allowed.running_long) {
          const session = sessions.get(job.lease.holder.session) ?? null;
          show.push(text.runningLong(job, session, live.generated_at_ms));
        }
      }
    }

    /** @type {RecentRun[]} */
    const finished = [];
    for (const run of [...live.recent].reverse()) {
      const key = runKey(run);
      const isNew = !this.#runs.has(key);
      this.#runs.add(key);
      // A killed run has the kill's notification or the popup's own Stopped card.
      if (!isNew || (run.end_cause !== null && run.end_cause !== "released")) {
        continue;
      }
      if (run.noise !== null) {
        if (allowed.benchmark_finished && run.lease_id !== null) {
          show.push(text.benchmarkFinished(run, run.noise));
        }
      } else if (allowed.job_finished) {
        finished.push(run);
      }
    }
    keepOnly(this.#runs, live.recent.map(runKey));
    if (finished.length > 0) {
      show.push(this.#finished(finished, sessions));
    }

    for (const notice of [...live.notices].reverse()) {
      const isNew = !this.#notices.has(notice.lease_id);
      this.#notices.add(notice.lease_id);
      if (
        isNew &&
        notice.cause === "killed" &&
        !this.#localKills.has(notice.lease_id) &&
        allowed.killed_outside_flyout
      ) {
        show.push(text.killedOutside(notice));
      }
    }
    keepOnly(this.#notices, live.notices.map((notice) => notice.lease_id));
  }

  /**
   * The first finish opens a window; later ones within it replace its notification with the
   * count.
   *
   * @param {RecentRun[]} runs
   * @param {Map<string, UiSession>} sessions
   * @returns {NotificationMessage}
   */
  #finished(runs, sessions) {
    const now = this.#now();
    if (
      this.#merged.length === 0 ||
      this.#mergeStart === null ||
      now - this.#mergeStart >= MERGE_WINDOW_MS
    ) {
      this.#merged = [];
      this.#mergeStart = now;
      this.#mergeWindows += 1;
    }
    this.#merged.push(...runs);
    const tag = `done-${this.#mergeWindows}`;
    const [only] = this.#merged;
    if (this.#merged.length > 1 || only === undefined) {
      return text.jobsFinished(this.#merged, tag);
    }
    const session = only.session === null ? null : (sessions.get(only.session) ?? null);
    return text.jobFinished(only, session, tag);
  }
}

/**
 * @param {ActiveJob} job
 * @param {number} nowMs
 * @returns {boolean}
 */
function isLong(job, nowMs) {
  const { lease } = job;
  const elapsedMs = nowMs - (lease.granted_at_ms ?? lease.requested_at_ms);
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
