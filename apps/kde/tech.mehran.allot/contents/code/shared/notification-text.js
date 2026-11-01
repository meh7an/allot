// Generated from apps/gnome/allot@mehran.tech/lib/notification-text.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import * as format from "./format.js";
/** @import { ActiveJob, NoiseReport, Notice, RecentRun, UiSession } from "../contract.g.mjs" */
/**
 * @typedef {"service-stopped" | "running-long" | "benchmark-finished" | "job-finished" |
 *   "killed-outside"} NotificationKind
 */
/** @typedef {"start-service" | "open-logs" | "show"} NotificationButton */
/**
 * One notification to show, in the extension's words.
 *
 * @typedef {object} NotificationMessage
 * @property {NotificationKind} kind
 * @property {string} tag
 *   Names the notification; showing another with the same tag replaces it.
 * @property {string} title
 * @property {string} body
 * @property {NotificationButton[]} buttons
 *   Primary first.
 * @property {string | null} target
 *   The popup row that Show or a click scrolls to (`job:<lease>` or `recent`), or `null` for the
 *   top.
 */
/** The one Service stopped notification, withdrawn once the service is back. */
export const SERVICE_TAG = "service";
/** The Recently finished card's row key. */
export const RECENT_TARGET = "recent";
/**
 * An active or queued card's row key.
 *
 * @param {number} lease
 * @returns {string}
 */
export function jobTarget(lease) {
    return `job:${lease}`;
}
/**
 * @param {number} sessions
 * @returns {NotificationMessage}
 */
export function serviceStopped(sessions) {
    return {
        kind: "service-stopped",
        tag: SERVICE_TAG,
        title: "Service stopped",
        body: `Heavy commands now run unchecked. ${connected(sessions)}`,
        buttons: ["start-service", "open-logs"],
        target: null,
    };
}
/**
 * @param {ActiveJob} job The job past 1.5 times its estimate.
 * @param {UiSession | null} session Its session, whose folder stands in when the lease has none.
 * @param {number} nowMs The state's time.
 * @returns {NotificationMessage}
 */
export function runningLong(job, session, nowMs) {
    var _a;
    const { lease } = job;
    const elapsed = Math.max(0, (nowMs - ((_a = lease.granted_at_ms) !== null && _a !== void 0 ? _a : lease.requested_at_ms)) / 1000);
    const times = `${format.duration(elapsed)} so far against a ` +
        `~${format.duration(lease.profile.est_secs)} estimate`;
    return {
        kind: "running-long",
        tag: `long-${lease.id}`,
        title: `${lease.label} is running long`,
        body: joined(times, lease.holder.session, lease.cwd, session),
        buttons: ["show"],
        target: jobTarget(lease.id),
    };
}
/**
 * @param {RecentRun} run The finished benchmark run.
 * @param {NoiseReport} noise Its background load.
 * @returns {NotificationMessage}
 */
export function benchmarkFinished(run, noise) {
    var _a;
    const forced = noise.quiet_at_start ? "" : "not quiet at start · ";
    const avg = format.oneDecimal(noise.avg_cpu_pct);
    const peak = format.oneDecimal(noise.peak_cpu_pct);
    return {
        kind: "benchmark-finished",
        tag: `bench-${(_a = run.lease_id) !== null && _a !== void 0 ? _a : ""}`,
        title: `Benchmark finished in ${format.duration(ranSecs(run))}`,
        body: `Background noise avg ${avg}%, peak ${peak}% CPU · ${forced}${run.command}`,
        buttons: [],
        target: run.lease_id === null ? null : jobTarget(run.lease_id),
    };
}
/**
 * @param {RecentRun} run The finished run.
 * @param {UiSession | null} session Its session, whose folder stands in when the run has none.
 * @param {string} tag The tag of this merge window.
 * @returns {NotificationMessage}
 */
export function jobFinished(run, session, tag) {
    var _a;
    return {
        kind: "job-finished",
        tag,
        title: `${run.command} finished`,
        body: joined(format.duration(ranSecs(run)), (_a = run.session) !== null && _a !== void 0 ? _a : "", run.cwd, session),
        buttons: ["show"],
        target: RECENT_TARGET,
    };
}
/**
 * Several finishes within the merge window: `3 jobs finished` with the first two commands.
 *
 * @param {RecentRun[]} runs The window's runs, oldest first; at least two.
 * @param {string} tag The tag of this merge window.
 * @returns {NotificationMessage}
 */
export function jobsFinished(runs, tag) {
    const [first, second] = runs.map((run) => run.command);
    const commands = runs.length === 2
        ? `${first} and ${second}`
        : `${first}, ${second} and ${runs.length - 2} more`;
    return {
        kind: "job-finished",
        tag,
        title: `${runs.length} jobs finished`,
        body: commands,
        buttons: ["show"],
        target: RECENT_TARGET,
    };
}
/**
 * @param {Notice} notice
 * @returns {NotificationMessage}
 */
export function killedOutside(notice) {
    return {
        kind: "killed-outside",
        tag: `killed-${notice.lease_id}`,
        title: `${notice.command} was stopped`,
        body: "Killed from the command line · Claude was notified",
        buttons: [],
        target: null,
    };
}
/**
 * @param {number} sessions
 * @returns {string}
 */
function connected(sessions) {
    switch (sessions) {
        case 0:
            return "No Claude sessions are connected.";
        case 1:
            return "1 Claude session is connected.";
        default:
            return `${sessions} Claude sessions are connected.`;
    }
}
/**
 * @param {RecentRun} run
 * @returns {number}
 */
function ranSecs(run) {
    return Math.max(0, (run.ended_at_ms - run.started_at_ms) / 1000);
}
/**
 * `<first> · <session> · <project>`, leaving out what is unknown. The job's own folder comes
 * first, as the notification is about that job.
 *
 * @param {string} first
 * @param {string} session
 * @param {string} cwd
 * @param {UiSession | null} live
 * @returns {string}
 */
function joined(first, session, cwd, live) {
    var _a;
    const parts = [first];
    if (session !== "") {
        parts.push(format.shortId(session));
    }
    const project = format.project(cwd === "" ? ((_a = live === null || live === void 0 ? void 0 : live.cwd) !== null && _a !== void 0 ? _a : null) : cwd);
    if (project !== "") {
        parts.push(project);
    }
    return parts.join(" · ");
}
