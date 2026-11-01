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
export const START_TIMEOUT_MS = 10_000;

/**
 * Polls the daemon on the `PollSchedule` and keeps the last state it served, which the
 * service-down view shows dimmed. Start Service launches allotd and reports it as starting until
 * it answers or `START_TIMEOUT_MS` passes.
 */
export class DaemonPoller {
  /**
   * Called after every poll.
   *
   * @type {((result: PollResult) => void) | null}
   */
  onUpdate = null;

  /** @type {PollResult | null} */
  latest = null;

  /**
   * The newest state allotd served, kept while it is down.
   *
   * @type {UiState | null}
   */
  lastKnown = null;

  /**
   * Why the last Start Service failed: the launch itself, or allotd not answering in time.
   * Cleared by the next start and once allotd answers.
   *
   * @type {string | null}
   */
  startProblem = null;

  #client;
  #timers;
  #launcher;
  #now;
  /** @type {number | null} */
  #startingUntil = null;
  #schedule = new PollSchedule();
  #popupOpen = false;
  #looping = false;
  #stopped = false;
  #wakePending = false;
  /** @type {unknown} */
  #timer = null;
  /** @type {(() => void) | null} */
  #wake = null;

  /**
   * @param {DaemonClient} client
   * @param {Timers} timers
   * @param {object} [options]
   * @param {ServiceLauncher | null} [options.launcher] Starts allotd for Start Service; without
   *   one, Start Service only polls.
   * @param {() => number} [options.now] The local clock of the starting window, in milliseconds.
   */
  constructor(client, timers, options = {}) {
    this.#client = client;
    this.#timers = timers;
    this.#launcher = options.launcher ?? null;
    this.#now = options.now ?? (() => Date.now());
  }

  /**
   * Launches allotd and polls quickly until it answers or `START_TIMEOUT_MS` passes.
   *
   * @returns {Promise<string | null>} `null` when allotd was launched, else why not.
   */
  async startService() {
    const problem = (await this.#launcher?.start()) ?? null;
    // A failed launch polls too, so whoever shows the poll shows the problem at once.
    this.startProblem = problem;
    if (problem === null) {
      this.#startingUntil = this.#now() + START_TIMEOUT_MS;
    }
    this.pollNow();
    return problem;
  }

  /**
   * Asks allotd to stop, then polls at once so the icon and popup show it stopped.
   *
   * @returns {Promise<string | null>} `null` when allotd accepted, else why not.
   */
  async stopService() {
    const problem = await this.#client.shutdown();
    this.pollNow();
    return problem;
  }

  /**
   * Switches between the open and closed cadence; opening also polls at once.
   *
   * @param {boolean} open
   */
  setPopupOpen(open) {
    this.#popupOpen = open;
    if (open) {
      this.pollNow();
    }
  }

  /**
   * Polls at once and restarts the backoff. A wake that arrives mid-poll ends the next wait
   * instead of being lost.
   */
  pollNow() {
    this.#schedule.reset();
    if (this.#wake === null) {
      this.#wakePending = true;
    } else {
      this.#endWait();
    }
  }

  /** Polls until `stop`. */
  async start() {
    if (this.#looping) {
      return;
    }
    this.#looping = true;
    this.#stopped = false;
    while (!this.#stopped) {
      let delay;
      try {
        delay = await this.pollOnce();
      } catch (error) {
        if (error instanceof CancelledError) {
          break;
        }
        throw error;
      }
      await this.#wait(delay);
    }
    this.#looping = false;
  }

  /**
   * Ends the loop and drops a poll in flight; the caller cancels its request, since the
   * transport owns it.
   */
  stop() {
    this.#stopped = true;
    this.#endWait();
  }

  /**
   * @returns {Promise<number>} Milliseconds to wait before the next poll.
   */
  async pollOnce() {
    const polled = await this.#client.poll();
    if (this.#stopped) {
      throw new CancelledError();
    }
    const result = this.#whileStarting(polled);
    const delay = this.#schedule.next(this.#popupOpen, result.status);
    if (result.state !== null) {
      this.lastKnown = result.state;
    }
    this.latest = result;
    this.onUpdate?.(result);
    return delay;
  }

  /**
   * A launched allotd takes a moment to bind its port; until then it looks stopped.
   *
   * @param {PollResult} result
   * @returns {PollResult}
   */
  #whileStarting(result) {
    if (result.status !== "stopped") {
      this.#startingUntil = null;
      this.startProblem = null;
      return result;
    }
    if (this.#startingUntil === null) {
      return result;
    }
    if (this.#now() < this.#startingUntil) {
      return { ...result, status: "starting" };
    }
    this.#startingUntil = null;
    const detail =
      `allotd did not answer within ${START_TIMEOUT_MS / 1000} s; ` + "see logs/allotd.log";
    this.startProblem = detail;
    return { ...result, status: "stopped", detail };
  }

  /**
   * @param {number} ms
   * @returns {Promise<void>}
   */
  #wait(ms) {
    if (this.#wakePending || this.#stopped) {
      this.#wakePending = false;
      return Promise.resolve();
    }
    return new Promise((resolve) => {
      this.#wake = resolve;
      this.#timer = this.#timers.setTimeout(() => {
        this.#timer = null;
        this.#endWait();
      }, ms);
    });
  }

  #endWait() {
    if (this.#timer !== null) {
      this.#timers.clearTimeout(this.#timer);
      this.#timer = null;
    }
    const wake = this.#wake;
    this.#wake = null;
    wake?.();
  }
}
