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

  #client;
  #timers;
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
   */
  constructor(client, timers) {
    this.#client = client;
    this.#timers = timers;
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
    const result = await this.#client.poll();
    if (this.#stopped) {
      throw new CancelledError();
    }
    const delay = this.#schedule.next(this.#popupOpen, result.status);
    if (result.state !== null) {
      this.lastKnown = result.state;
    }
    this.latest = result;
    this.onUpdate?.(result);
    return delay;
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
