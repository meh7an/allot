/** @import { KillResponse, RemoveResponse } from "../contract.g.mjs" */

/**
 * What allotd answered a Kill or Remove from the popup.
 *
 * @typedef {object} StopResult
 * @property {boolean} ended
 *   allotd ended the lease, and its session will be told.
 * @property {string} text
 *   Ended: a note for the Stopped line, or empty. Otherwise why not.
 * @property {number | null} endedAtMs
 *   Ended: when the lease ended, in the daemon's clock.
 */

/**
 * A kill ends the lease even when part of the tree, or all of it, is left running; the note
 * says so.
 *
 * @param {KillResponse} response
 * @returns {StopResult}
 */
export function killed(response) {
  let note = "";
  if (response.untouched_reason !== null) {
    note = "nothing was running";
  } else if (response.failed.length === 1) {
    note = "1 process still running";
  } else if (response.failed.length > 1) {
    note = `${response.failed.length} processes still running`;
  }
  return { ended: true, text: note, endedAtMs: response.lease.ended_at_ms };
}

/**
 * @param {RemoveResponse} response
 * @returns {StopResult}
 */
export function removed(response) {
  return { ended: true, text: "", endedAtMs: response.lease.ended_at_ms };
}

/**
 * @param {string} why
 * @returns {StopResult}
 */
export function refused(why) {
  return { ended: false, text: why, endedAtMs: null };
}
