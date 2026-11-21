// Generated from apps/gnome/allot@mehran.tech/lib/service-banner.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/** @import { ServiceStatus } from "./results.js" */
/** @import { UiState } from "../contract.g.mjs" */
/**
 * - `none`: no button, because starting allotd again would not help.
 * - `start`: the accent Start Service button.
 * - `restart`: the accent Restart Service button, for an allotd the panel cannot reach.
 * - `starting`: the muted, disabled Starting… that follows a press.
 *
 * @typedef {"none" | "start" | "restart" | "starting"} BannerButton
 */
/**
 * The banner at the top of the popup while the service is down, and the caption above the last
 * known cards.
 *
 * @typedef {object} ServiceBanner
 * @property {string} text
 *   The semibold banner line.
 * @property {string} problem
 *   Why the last Start Service failed, or empty.
 * @property {BannerButton} button
 * @property {string} caption
 *   For example `Last known · 6:52 AM`; empty without a last known state, which also means no
 *   cards.
 */
export const STOPPED_TEXT = "Service stopped · commands run unchecked";
export const OFF_BUS_TEXT = "Service runs without the session bus · restart it outside the sandbox";
export const INCOMPATIBLE_TEXT = "Service needs an update · run allot install";
/**
 * The banner button's title, in title case.
 *
 * @param {BannerButton} button
 * @returns {string}
 */
export function bannerButtonText(button) {
    switch (button) {
        case "restart":
            return "Restart Service";
        case "starting":
            return "Starting…";
        case "none":
        case "start":
            return "Start Service";
    }
}
/**
 * @param {ServiceStatus} status The service status of the latest poll.
 * @param {UiState | null} lastKnown The last state allotd served, if any.
 * @param {string | null} startProblem Why the last Start Service failed, if it did.
 * @param {(ms: number) => string} timeOfDay A daemon time as the user's short time of day.
 * @returns {ServiceBanner | null} `null` while the service runs.
 */
export function serviceBanner(status, lastKnown, startProblem, timeOfDay) {
    const caption = lastKnown === null ? "" : `Last known · ${timeOfDay(lastKnown.generated_at_ms)}`;
    switch (status) {
        case "running":
            return null;
        case "starting":
            return { text: STOPPED_TEXT, problem: "", button: "starting", caption };
        case "off_bus":
            return { text: OFF_BUS_TEXT, problem: startProblem !== null && startProblem !== void 0 ? startProblem : "", button: "restart", caption };
        case "incompatible":
            return { text: INCOMPATIBLE_TEXT, problem: "", button: "none", caption };
        case "stopped":
            return { text: STOPPED_TEXT, problem: startProblem !== null && startProblem !== void 0 ? startProblem : "", button: "start", caption };
    }
}
