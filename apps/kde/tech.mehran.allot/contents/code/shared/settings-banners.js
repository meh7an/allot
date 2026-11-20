// Generated from apps/gnome/allot@mehran.tech/lib/settings-banners.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import { changeValueText, joined } from "./settings-text.js";
/**
 * @import { Conflict, ConflictEdit, SettingsSession } from "./settings-session.js"
 */
// Which banners a Settings page shows for the session's state (DESIGN.md section 7). The Panel
// page shows none of allotd's: it keeps working in every state.
/** @typedef {"info" | "warning" | "error"} BannerSeverity */
/**
 * The buttons a banner can carry.
 *
 * @typedef {(
 *   | "start_service"
 *   | "restart_service"
 *   | "try_now"
 *   | "open_logs"
 *   | "open_file"
 *   | "check_again"
 *   | "apply_mine"
 *   | "keep_file_values"
 *   | "save_again"
 * )} BannerAction
 */
/**
 * A lasting state at the top of a Settings page.
 *
 * @typedef {object} SettingsBanner
 * @property {BannerSeverity} severity
 * @property {string} message What happened and what it means.
 * @property {BannerAction[]} actions Its buttons, the first one primary.
 */
export const PANEL_PAGE = "panel";
export const SERVICE_PAGE = "service";
const PORT_KEY = "daemon.port";
/**
 * A banner button's title, in title case.
 *
 * @param {BannerAction} action
 * @returns {string}
 */
export function bannerActionLabel(action) {
    switch (action) {
        case "start_service":
            return "Start Service";
        case "restart_service":
            return "Restart Service";
        case "try_now":
            return "Try Now";
        case "open_logs":
            return "Open Logs";
        case "open_file":
            return "Open File";
        case "check_again":
            return "Check Again";
        case "apply_mine":
            return "Apply My Change";
        case "keep_file_values":
            return "Keep File's Values";
        case "save_again":
            return "Try Again";
    }
}
/**
 * The banners of a page, most lasting first.
 *
 * @param {SettingsSession} session
 * @param {string} pageId The page shown.
 * @param {number} port The port this panel reaches allotd on.
 * @param {(value: number) => string} number Formats a number for the user's locale.
 * @returns {SettingsBanner[]}
 */
export function settingsBanners(session, pageId, port, number) {
    var _a;
    if (pageId === PANEL_PAGE) {
        return [];
    }
    const unanswered = unansweredBanner(session, port);
    if (unanswered !== null) {
        return [unanswered];
    }
    if (session.status !== "ready") {
        return [];
    }
    /** @type {SettingsBanner[]} */
    const banners = [];
    if (session.fileError !== null) {
        banners.push({
            severity: "error",
            message: `config.toml has an error: ${sentence(session.fileError)} The service keeps using ` +
                "its last good settings. Fix the file to change settings here.",
            actions: ["open_file", "check_again"],
        });
    }
    if (session.conflict !== null) {
        banners.push({
            severity: "warning",
            message: "config.toml changed outside Settings. Showing the file's values. " +
                notSaved(session.conflict, number),
            actions: ["apply_mine", "keep_file_values"],
        });
    }
    if (session.writeProblem !== null) {
        banners.push({
            severity: "warning",
            message: `Your changes are not saved yet. ${sentence(session.writeProblem)}`,
            actions: ["save_again"],
        });
    }
    const newPort = (_a = session.setting(PORT_KEY)) === null || _a === void 0 ? void 0 : _a.saved;
    if (pageId === SERVICE_PAGE &&
        session.pendingRestart.includes(PORT_KEY) &&
        typeof newPort === "number") {
        banners.push({
            severity: "info",
            message: `Restart the service to use port ${String(newPort)}.`,
            actions: ["restart_service"],
        });
    }
    return banners;
}
/**
 * The line under Port while a new port waits for a restart, or `null`.
 *
 * @param {SettingsSession} session
 * @param {string} key
 * @returns {string | null}
 */
export function restartNote(session, key) {
    if (!session.pendingRestart.includes(key) || session.listeningPort === null) {
        return null;
    }
    return `Restart needed. The service still listens on ${String(session.listeningPort)}.`;
}
/**
 * @param {SettingsSession} session
 * @param {number} port
 * @returns {SettingsBanner | null}
 */
function unansweredBanner(session, port) {
    switch (session.status) {
        case "stopped":
            return {
                severity: "info",
                message: "The service is stopped. Start it to change these settings.",
                actions: ["start_service"],
            };
        case "down":
            return {
                severity: "warning",
                message: `The service isn't answering on port ${String(port)}. Trying again every 5 s.`,
                actions: ["try_now", "open_logs"],
            };
        case "token":
            return {
                severity: "error",
                message: "The service refused this panel's key. Restart the service to make a new one.",
                actions: ["restart_service"],
            };
        case "failed":
            return {
                severity: "error",
                message: `The service sent an answer Settings can't use. ${sentence(session.detail)}`,
                actions: ["try_now", "open_logs"],
            };
        case "loading":
        case "ready":
            return null;
    }
}
/**
 * "Your change to Safety margin (15 %) was not saved."
 *
 * @param {Conflict} conflict
 * @param {(value: number) => string} number
 * @returns {string}
 */
function notSaved(conflict, number) {
    const edits = conflict.edits.map((edit) => editText(edit, number));
    return edits.length === 1
        ? `Your change to ${edits[0]} was not saved.`
        : `Your changes to ${joined(edits)} were not saved.`;
}
/**
 * @param {ConflictEdit} edit
 * @param {(value: number) => string} number
 * @returns {string}
 */
function editText(edit, number) {
    const change = edit.changes[0];
    if (change === undefined) {
        return edit.setting.label;
    }
    return `${edit.setting.label} (${changeValueText(edit.setting, change, number)})`;
}
/**
 * allotd's reasons come without a full stop; the banner's next sentence needs one.
 *
 * @param {string | null} text
 * @returns {string}
 */
function sentence(text) {
    const trimmed = (text !== null && text !== void 0 ? text : "").trim();
    return trimmed === "" || trimmed.endsWith(".") ? trimmed : `${trimmed}.`;
}
