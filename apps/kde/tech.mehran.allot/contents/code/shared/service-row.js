// Generated from apps/gnome/allot@mehran.tech/lib/service-row.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import { INCOMPATIBLE_TEXT } from "./service-banner.js";
/**
 * @import { ServiceStatus } from "./results.js"
 * @import { SessionStatus } from "./settings-session.js"
 */
// The status row at the top of the Settings window's Service page.
/**
 * How a panel reaches allotd, which the texts that blame the connection name: GNOME talks over
 * the session bus, KDE over a loopback port.
 *
 * @typedef {{ kind: "port", port: number } | { kind: "bus" }} Reach
 */
/**
 * - `stop`: Stop Service…, which asks first while leases are active.
 * - `start`: Start Service.
 * - `restart`: Restart Service, for an allotd the panel cannot reach.
 * - `starting`: the disabled, busy Start that follows a press.
 *
 * @typedef {"stop" | "start" | "restart" | "starting"} ServiceButton
 */
/**
 * @typedef {object} ServiceRow
 * @property {string} text The state in a sentence.
 * @property {ServiceButton} button The button beside it.
 * @property {boolean} opensLogs Whether Open Logs shows too.
 */
export const STOPPED_BY_YOU_TEXT = "Stopped. Heavy commands run without leases until you start it.";
export const OFF_BUS_ROW_TEXT = "Runs without the session bus, so this panel cannot reach it. Restart it outside the sandbox.";
export const STARTING_TEXT = "Starting…";
/**
 * @param {ServiceStatus} status The latest poll.
 * @param {SessionStatus} session The settings session's status, which knows the stop record.
 * @param {string | null} version allotd's version, when it is known.
 * @param {Reach} reach How this panel reaches allotd.
 * @returns {ServiceRow}
 */
export function serviceRow(status, session, version, reach) {
    switch (status) {
        case "running": {
            const parts = ["Running"];
            if (version !== null) {
                parts.push(`version ${version}`);
            }
            if (reach.kind === "port") {
                parts.push(`port ${String(reach.port)}`);
            }
            return { text: parts.join(" · "), button: "stop", opensLogs: false };
        }
        case "incompatible":
            return { text: INCOMPATIBLE_TEXT, button: "stop", opensLogs: true };
        case "starting":
            return { text: STARTING_TEXT, button: "starting", opensLogs: false };
        case "off_bus":
            return { text: OFF_BUS_ROW_TEXT, button: "restart", opensLogs: true };
        case "stopped":
            return session === "stopped"
                ? { text: STOPPED_BY_YOU_TEXT, button: "start", opensLogs: false }
                : { text: notAnswering(reach), button: "start", opensLogs: true };
    }
}
/**
 * @param {Reach} reach
 * @returns {string}
 */
function notAnswering(reach) {
    return reach.kind === "port"
        ? `Not answering on port ${String(reach.port)}.`
        : "Not on the session bus.";
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
        case "restart":
            return "Restart Service";
        case "starting":
            return STARTING_TEXT;
    }
}
/**
 * The service status of a window that only reads settings: a GNOME preferences process has no
 * poller of its own.
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
        case "off_bus":
            return "off_bus";
        case "stopped":
        case "down":
            return "stopped";
        case "key":
        case "outdated":
        case "failed":
            return "incompatible";
    }
}
