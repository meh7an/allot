// Generated from apps/gnome/allot@mehran.tech/lib/snapshot.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import * as format from "./format.js";
/** @import { Lease, QueuedJob, UiSession, UiState } from "../contract.g.mjs" */
/** @import { ServiceStatus } from "./client.js" */
/**
 * - `idle`: the dimmed mark.
 * - `busy`: the solid mark; a job runs, or a benchmark runs alone.
 * - `error`: the error icon; the service is not serving state.
 *
 * @typedef {"idle" | "busy" | "error"} HeaderGlyph
 */
/** @typedef {"benchmark" | "running" | "waiting"} JobGroupKind */
/**
 * Where the live benchmark stands, as the summary words it.
 *
 * - `draining`: queued while other leases finish, or queued behind other requests.
 * - `waiting-for-quiet`: the machine has drained and its background load is being measured.
 * - `running-alone`: granted; it has the machine to itself.
 *
 * @typedef {"draining" | "waiting-for-quiet" | "running-alone"} BenchmarkPhase
 */
/**
 * @typedef {object} PanelHeader
 * @property {HeaderGlyph} glyph
 * @property {string} summary
 */
/**
 * The strip at the top of a card that names the job's session.
 *
 * @typedef {object} SessionStrip
 * @property {string} shortId
 *   The session ID's first 8 characters.
 * @property {string} project
 *   The project folder name, or empty.
 * @property {string} projectPath
 *   The full working directory, for the project's tooltip.
 * @property {string} usage
 *   Live session usage, `also running` on a queued card of a session that runs a job, or
 *   empty when the session is not registered.
 */
/**
 * One job row: an active lease or a queued request.
 *
 * @typedef {object} JobRow
 * @property {number} id
 *   The lease ID.
 * @property {string} command
 *   The command, from the lease's label.
 * @property {SessionStrip} session
 * @property {boolean} isQueued
 *   Waiting in the queue rather than running.
 */
/**
 * @typedef {object} JobGroup
 * @property {JobGroupKind} kind
 * @property {string} heading
 * @property {JobRow[]} jobs
 */
/**
 * The plain `Idle · N  ids  usage` line after the groups.
 *
 * @typedef {object} IdleLine
 * @property {string} label
 *   For example `Idle · 2`.
 * @property {string} ids
 *   Short session IDs separated by two spaces.
 * @property {string} usage
 *   Live usage, shown only when exactly one session is idle.
 */
/**
 * Everything the popup header and job list show for one poll, built by a pure function so that
 * every string is testable. The Windows tray app's `FlyoutSnapshot` without what later
 * milestones add: card details, stops, recent runs and the first-run card.
 *
 * @typedef {object} PanelSnapshot
 * @property {PanelHeader} header
 * @property {JobGroup[]} groups
 *   Benchmark, Running and Waiting, in that order, each only when it has jobs.
 * @property {IdleLine | null} idle
 *   The idle line, or `null` when no session is idle.
 * @property {boolean} showNothing
 *   The "No jobs running or waiting." line: the service runs and has no jobs.
 * @property {BenchmarkPhase | null} benchmark
 *   The live benchmark's phase, or `null` without one.
 * @property {boolean} isLastKnown
 *   The jobs come from the last state allotd served before it went away; they show dimmed.
 */
/**
 * @param {ServiceStatus} status The service status of the latest poll.
 * @param {UiState | null} state The latest state, or the last known one while allotd is not
 *   running.
 * @returns {PanelSnapshot}
 */
export function panelSnapshot(status, state) {
    var _a, _b, _c;
    const lastKnown = status !== "running";
    /** @type {Map<string, UiSession>} */
    const sessions = new Map();
    for (const session of (_a = state === null || state === void 0 ? void 0 : state.sessions) !== null && _a !== void 0 ? _a : []) {
        if (!sessions.has(session.session)) {
            sessions.set(session.session, session);
        }
    }
    const active = (_b = state === null || state === void 0 ? void 0 : state.active) !== null && _b !== void 0 ? _b : [];
    const queue = (_c = state === null || state === void 0 ? void 0 : state.queue) !== null && _c !== void 0 ? _c : [];
    const runningSessions = new Set(active.map((job) => job.lease.holder.session));
    const busySessions = new Set([
        ...runningSessions,
        ...queue.map((job) => job.lease.holder.session),
    ]);
    /** @type {JobRow[]} */
    const benchmark = [];
    /** @type {JobRow[]} */
    const running = [];
    /** @type {JobRow[]} */
    const waiting = [];
    let benchmarkRuns = false;
    for (const { lease } of active) {
        const row = jobRow(lease, strip(lease, sessions, false), false);
        if (isBenchmark(lease)) {
            benchmark.push(row);
            benchmarkRuns = true;
        }
        else {
            running.push(row);
        }
    }
    /** @type {QueuedJob | null} */
    let firstQueuedBenchmark = null;
    for (const job of queue) {
        const { lease } = job;
        const alsoRunning = runningSessions.has(lease.holder.session);
        const row = jobRow(lease, strip(lease, sessions, alsoRunning), true);
        if (isBenchmark(lease)) {
            benchmark.push(row);
            firstQueuedBenchmark !== null && firstQueuedBenchmark !== void 0 ? firstQueuedBenchmark : (firstQueuedBenchmark = job);
        }
        else {
            waiting.push(row);
        }
    }
    /** @type {JobGroup[]} */
    const groups = [];
    if (benchmark.length > 0) {
        groups.push({ kind: "benchmark", heading: "Benchmark", jobs: benchmark });
    }
    if (running.length > 0) {
        groups.push({ kind: "running", heading: `Running · ${running.length}`, jobs: running });
    }
    if (waiting.length > 0) {
        groups.push({ kind: "waiting", heading: `Waiting · ${waiting.length}`, jobs: waiting });
    }
    /** @type {BenchmarkPhase | null} */
    let phase = null;
    if (benchmarkRuns) {
        phase = "running-alone";
    }
    else if (firstQueuedBenchmark !== null) {
        phase = queuedPhase(firstQueuedBenchmark);
    }
    return {
        header: header(status, running.length, waiting.length, phase),
        groups,
        idle: idleLine([...sessions.values()], busySessions),
        showNothing: !lastKnown && groups.length === 0,
        benchmark: phase,
        isLastKnown: lastKnown,
    };
}
/**
 * @param {ServiceStatus} status
 * @param {number} running
 * @param {number} waiting
 * @param {BenchmarkPhase | null} phase
 * @returns {PanelHeader}
 */
function header(status, running, waiting, phase) {
    switch (status) {
        case "starting":
            return { glyph: "error", summary: "Starting service…" };
        case "stopped":
            return { glyph: "error", summary: "Service stopped" };
        case "incompatible":
            return { glyph: "error", summary: "Service needs an update" };
        case "running":
            break;
    }
    const glyph = running > 0 || phase === "running-alone" ? "busy" : "idle";
    return { glyph, summary: summary(running, waiting, phase) };
}
/**
 * @param {number} running
 * @param {number} waiting
 * @param {BenchmarkPhase | null} phase
 * @returns {string}
 */
function summary(running, waiting, phase) {
    switch (phase) {
        case "draining":
            return running > 0 ? `${running} running · benchmark waiting` : "Benchmark waiting";
        case "waiting-for-quiet":
            return "Benchmark waiting for quiet";
        case "running-alone":
            return "Benchmark running alone";
        case null:
            return counts(running, waiting);
    }
}
/**
 * @param {number} running
 * @param {number} waiting
 * @returns {string}
 */
function counts(running, waiting) {
    const parts = [];
    if (running > 0) {
        parts.push(`${running} running`);
    }
    if (waiting > 0) {
        parts.push(`${waiting} waiting`);
    }
    return parts.length === 0 ? "Idle" : parts.join(" · ");
}
/**
 * A queued benchmark either drains the machine or, once it is at the head and drained,
 * measures the background load.
 *
 * @param {QueuedJob} job
 * @returns {BenchmarkPhase}
 */
function queuedPhase(job) {
    var _a;
    switch ((_a = job.verdict.benchmark) === null || _a === void 0 ? void 0 : _a.phase) {
        case "measuring":
        case "noisy":
            return "waiting-for-quiet";
        default:
            return "draining";
    }
}
/**
 * @param {UiSession[]} sessions
 * @param {Set<string>} busy
 * @returns {IdleLine | null}
 */
function idleLine(sessions, busy) {
    const idle = sessions.filter((session) => !busy.has(session.session));
    const [only] = idle;
    if (only === undefined) {
        return null;
    }
    return {
        label: `Idle · ${idle.length}`,
        ids: idle.map((session) => format.shortId(session.session)).join("  "),
        usage: idle.length === 1 ? format.usage(only.usage) : "",
    };
}
/**
 * @param {Lease} lease
 * @param {SessionStrip} session
 * @param {boolean} isQueued
 * @returns {JobRow}
 */
function jobRow(lease, session, isQueued) {
    return { id: lease.id, command: lease.label, session, isQueued };
}
/**
 * @param {Lease} lease
 * @returns {boolean}
 */
function isBenchmark(lease) {
    return lease.kind === "benchmark_exclusive";
}
/**
 * A queued card of a session that also runs a job says so instead of repeating the session's
 * usage, which its running card already shows.
 *
 * @param {Lease} lease
 * @param {Map<string, UiSession>} sessions
 * @param {boolean} alsoRunning
 * @returns {SessionStrip}
 */
function strip(lease, sessions, alsoRunning) {
    const key = lease.holder.session;
    const session = sessions.get(key);
    const cwd = (session === null || session === void 0 ? void 0 : session.cwd) || lease.cwd;
    let usage = "";
    if (alsoRunning) {
        usage = "also running";
    }
    else if (session !== undefined) {
        usage = format.usage(session.usage);
    }
    return {
        shortId: format.shortId(key),
        project: format.project(cwd),
        projectPath: cwd,
        usage,
    };
}
