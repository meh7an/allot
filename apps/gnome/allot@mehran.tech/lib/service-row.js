import { INCOMPATIBLE_TEXT } from "./service-banner.js";

/**
 * @import { ServiceStatus } from "./client.js"
 * @import { SessionStatus } from "./settings-session.js"
 */

// The status row at the top of the Settings window's Service page (DESIGN.md section 4,
// Service).

/**
 * - `stop`: Stop Service…, which asks first while leases are active.
 * - `start`: Start Service.
 * - `starting`: the disabled, busy Start that follows a press.
 *
 * @typedef {"stop" | "start" | "starting"} ServiceButton
 */

/**
 * @typedef {object} ServiceRow
 * @property {string} text The state in a sentence.
 * @property {ServiceButton} button The button beside it.
 * @property {boolean} opensLogs Whether Open Logs shows too.
 */

export const STOPPED_BY_YOU_TEXT =
  "Stopped. Heavy commands run without leases until you start it.";

export const STARTING_TEXT = "Starting…";

/**
 * @param {ServiceStatus} status The latest poll.
 * @param {SessionStatus} session The settings session's status, which knows the stop record.
 * @param {string | null} version allotd's version, when it is known.
 * @param {number} port The port allotd listens on, or this panel dials.
 * @returns {ServiceRow}
 */
export function serviceRow(status, session, version, port) {
  switch (status) {
    case "running": {
      const text =
        version === null
          ? `Running · port ${String(port)}`
          : `Running · version ${version} · port ${String(port)}`;
      return { text, button: "stop", opensLogs: false };
    }
    case "incompatible":
      return { text: INCOMPATIBLE_TEXT, button: "stop", opensLogs: true };
    case "starting":
      return { text: STARTING_TEXT, button: "starting", opensLogs: false };
    case "stopped":
      return session === "stopped"
        ? { text: STOPPED_BY_YOU_TEXT, button: "start", opensLogs: false }
        : { text: `Not answering on port ${String(port)}.`, button: "start", opensLogs: true };
  }
}

/**
 * The button's title, in title case.
 *
 * @param {ServiceButton} button
 * @returns {string}
 */
export function serviceButtonText(button) {
  switch (button) {
    case "stop":
      return "Stop Service…";
    case "start":
      return "Start Service";
    case "starting":
      return STARTING_TEXT;
  }
}

/**
 * The service status of a window that polls only the settings routes: a GNOME preferences
 * process has no `/ui/state` poller of its own.
 *
 * @param {SessionStatus} session
 * @param {boolean} starting Whether a start or restart from the window is still under way.
 * @returns {ServiceStatus | null} `null` before the first answer.
 */
export function sessionServiceStatus(session, starting) {
  if (starting) {
    return "starting";
  }
  switch (session) {
    case "loading":
      return null;
    case "ready":
      return "running";
    case "stopped":
    case "down":
      return "stopped";
    case "token":
    case "failed":
      return "incompatible";
  }
}
