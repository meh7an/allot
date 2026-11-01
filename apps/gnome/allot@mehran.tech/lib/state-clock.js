/** @import { UiState } from "../contract.g.mjs" */

/**
 * The daemon's time between polls: the latest state's `generated_at_ms` plus the time since it
 * arrived, so that elapsed and waited times tick every second without a new poll and without
 * trusting the local clock to match allotd's.
 */
export class StateClock {
  #now;
  #generatedAtMs = 0;
  /** @type {number | null} */
  #receivedAt = null;

  /** @param {() => number} now The local clock in milliseconds; the tests pass their own. */
  constructor(now) {
    this.#now = now;
  }

  /**
   * The current moment in the daemon's clock, or `null` while no live state is known.
   *
   * @returns {number | null}
   */
  get nowMs() {
    if (this.#receivedAt === null) {
      return null;
    }
    return this.#generatedAtMs + Math.max(0, Math.floor(this.#now() - this.#receivedAt));
  }

  /**
   * Takes the state of a poll; `null` stops the clock, as last known times stand still.
   *
   * @param {UiState | null} state
   */
  received(state) {
    if (state === null) {
      this.#receivedAt = null;
      return;
    }
    this.#generatedAtMs = state.generated_at_ms;
    this.#receivedAt = this.#now();
  }
}
