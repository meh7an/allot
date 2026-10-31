/** @import { ServiceStatus } from "./client.js" */

// Live data ticks at 1 Hz while the popup is open; the closed popup only drives the panel
// icon. While allotd is unreachable the wait doubles, capped lower while the popup is open, so a
// service started elsewhere shows up quickly. The same cadence as the Windows and macOS apps.

export const OPEN_INTERVAL_MS = 1000;
export const CLOSED_INTERVAL_MS = 5000;
export const STARTING_INTERVAL_MS = 500;
export const FIRST_RETRY_MS = 1000;
export const OPEN_RETRY_CAP_MS = 5000;
export const CLOSED_RETRY_CAP_MS = 30000;

const MAX_DOUBLINGS = 16;

/** How long to wait before the next poll. */
export class PollSchedule {
  #failures = 0;

  /**
   * @param {boolean} popupOpen
   * @param {ServiceStatus} status The status the poll just returned.
   * @returns {number} Milliseconds.
   */
  next(popupOpen, status) {
    switch (status) {
      case "running":
        this.#failures = 0;
        return popupOpen ? OPEN_INTERVAL_MS : CLOSED_INTERVAL_MS;
      case "starting":
        this.#failures = 0;
        return STARTING_INTERVAL_MS;
      case "stopped":
      case "incompatible": {
        const cap = popupOpen ? OPEN_RETRY_CAP_MS : CLOSED_RETRY_CAP_MS;
        const retry = FIRST_RETRY_MS * 2 ** Math.min(this.#failures, MAX_DOUBLINGS);
        this.#failures += 1;
        return Math.min(retry, cap);
      }
    }
  }

  /** Opening the popup retries at once and from the shortest wait. */
  reset() {
    this.#failures = 0;
  }
}
