/** @import { ServiceStatus } from "./client.js" */
/** @import { UiState } from "../contract.g.mjs" */

/**
 * - `stop`: allotd answers, so it can be stopped.
 * - `start`: allotd is down.
 * - `none`: allotd is starting; the item is shown disabled.
 *
 * @typedef {"stop" | "start" | "none"} ServiceAction
 */

/**
 * The panel button's service entry.
 *
 * @typedef {object} ServiceMenuItem
 * @property {string} title
 * @property {ServiceAction} action
 */

export const STOP_CONFIRM_TITLE = "Stop Allot?";

/**
 * @param {ServiceStatus} status
 * @returns {ServiceMenuItem}
 */
export function serviceMenuItem(status) {
  switch (status) {
    case "running":
    case "incompatible":
      return { title: "Stop Service…", action: "stop" };
    case "starting":
      return { title: "Starting Service…", action: "none" };
    case "stopped":
      return { title: "Start Service", action: "start" };
  }
}

/**
 * The confirmation shown before stopping allotd. Every Claude session's MCP server starts allotd
 * again on its next call, so the text says the stop lasts only until then.
 *
 * @param {UiState | null} state
 * @returns {string | null} `null` when no lease is running or waiting, so no question is needed.
 */
export function stopConfirmation(state) {
  const running = state?.active.length ?? 0;
  const waiting = state?.queue.length ?? 0;
  if (running === 0 && waiting === 0) {
    return null;
  }
  const parts = [];
  if (running > 0) {
    parts.push(running === 1 ? "1 job is running" : `${running} jobs are running`);
  }
  if (waiting > 0) {
    parts.push(waiting === 1 ? "1 request is waiting" : `${waiting} requests are waiting`);
  }
  return (
    `${parts.join(" and ")}.\n\n` +
    "Heavy commands run unchecked while the service is stopped. A Claude session " +
    "starts it again the next time it asks for resources."
  );
}
