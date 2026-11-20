// Generated from apps/gnome/allot@mehran.tech/lib/service-menu.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
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
export const STOP_CONFIRM_TITLE = "Stop the service?";
/** The menu item that opens the Settings window. */
export const SETTINGS_TITLE = "Settings";
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
 * The confirmation shown before stopping allotd. The stop lasts until the user starts allotd again
 * (the stop record).
 *
 * @param {UiState | null} state
 * @returns {string | null} `null` when no lease is running or waiting, so no question is needed.
 */
export function stopConfirmation(state) {
    var _a, _b;
    const running = (_a = state === null || state === void 0 ? void 0 : state.active.length) !== null && _a !== void 0 ? _a : 0;
    const waiting = (_b = state === null || state === void 0 ? void 0 : state.queue.length) !== null && _b !== void 0 ? _b : 0;
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
    return (`${parts.join(" and ")}.\n\n` +
        "Jobs keep running, and new heavy commands run without leases until you start the " +
        "service again.");
}
