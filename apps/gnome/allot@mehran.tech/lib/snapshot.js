import * as format from "./format.js";
import * as jobCard from "./job-card.js";

/** @import { Lease, UiSession, UiState } from "../contract.g.mjs" */
/** @import { ServiceStatus } from "./client.js" */
/** @import { JobCard } from "./job-card.js" */
/** @import { ShownJobs, StopView } from "./job-stops.js" */

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
 * One card: an active lease, a queued request, or a benchmark that finished a moment ago.
 *
 * @typedef {object} JobRow
 * @property {number} id
 *   The lease ID; a finished benchmark keeps its lease's, so its card replaces the running one
 *   in place.
 * @property {string} command
 *   The command, from the lease's label.
 * @property {SessionStrip} session
 * @property {boolean} isQueued
 *   Waiting in the queue rather than running.
 * @property {JobCard} card
 *   The job row's text and state at the snapshot's moment.
 * @property {boolean} isEnding
 *   Stopped or removed in this popup and still showing its final state, or a finished
 *   benchmark; it no longer counts in the summary.
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
 * One row of the Recently finished card.
 *
 * @typedef {object} RecentLine
 * @property {string} command
 *   The command as it ran.
 * @property {string} shortId
 *   The session's short ID, or empty for runs recorded before sessions were.
 * @property {string} meta
 *   Run time and age, for example `4:52 · 4 min ago`.
 */

/**
 * Everything the popup header and job list show for one moment, built by a pure function so
 * that every string is testable. The Windows tray app's `FlyoutSnapshot` without the first-run
 * card.
 *
 * @typedef {object} PanelSnapshot
 * @property {PanelHeader} header
 * @property {JobGroup[]} groups
 *   Benchmark, Running and Waiting, in that order, each only when it has jobs.
 * @property {IdleLine | null} idle
 *   The idle line, or `null` when no session is idle.
 * @property {RecentLine[]} recent
 *   Recently finished runs, newest first; only while no job runs or waits.
 * @property {boolean} showNothing
 *   The service runs and has no jobs. The popup shows Recently finished instead of the
 *   "No jobs running or waiting." line when there are recent runs.
 * @property {BenchmarkPhase | null} benchmark
 *   The live benchmark's phase, or `null` without one.
 * @property {boolean} isLastKnown
 *   The jobs come from the last state allotd served before it went away; they show dimmed and
 *   without actions.
 */

/**
 * @typedef {object} SnapshotOptions
 * @property {ShownJobs} [jobs]
 *   The jobs to show instead of the state's, with ended stops put back (`JobStops.merge`).
 * @property {Map<number, StopView>} [stops]
 *   Kills and removals of this popup by lease; ended ones show Stopped or Removed and no longer
 *   count. Ignored for last known state.
 * @property {number | null} [nowMs]
 *   The moment times are shown for, in the daemon's clock; the state's own time when absent.
 *   Last known state always shows its own time.
 */

// How long a benchmark that finished on its own keeps its card.
const FINISHED_HOLD_MS = 60_000;

/**
 * @param {ServiceStatus} status The service status of the latest poll.
 * @param {UiState | null} state The latest state, or the last known one while allotd is not
 *   running.
 * @param {SnapshotOptions} [options]
 * @returns {PanelSnapshot}
 */
export function panelSnapshot(status, state, options = {}) {
  const lastKnown = status !== "running";
  /** @type {Map<number, StopView>} */
  const stops = lastKnown ? new Map() : (options.stops ?? new Map());
  const generated = state?.generated_at_ms ?? 0;
  const now = lastKnown ? generated : (options.nowMs ?? generated);
  /** @type {Map<string, UiSession>} */
  const sessions = new Map();
  for (const session of state?.sessions ?? []) {
    if (!sessions.has(session.session)) {
      sessions.set(session.session, session);
    }
  }
  const shown = options.jobs ?? { active: state?.active ?? [], queue: state?.queue ?? [] };
  /** @param {Lease} lease */
  const isEnding = (lease) => stops.get(lease.id)?.phase === "ended";
  const stopOf = (/** @type {Lease} */ lease) => stops.get(lease.id) ?? null;
  const runningSessions = new Set(shown.active.map((job) => job.lease.holder.session));
  // A stopped or removed benchmark no longer holds anything back.
  let benchmarkAhead = shown.active.some((job) => isBenchmark(job.lease) && !isEnding(job.lease));
  const othersActive = shown.active.some((job) => !isEnding(job.lease));

  /** @type {JobRow[]} */
  const benchmark = [];
  /** @type {JobRow[]} */
  const running = [];
  /** @type {JobRow[]} */
  const waiting = [];
  for (const job of shown.active) {
    const { lease } = job;
    const row = {
      id: lease.id,
      command: lease.label,
      session: strip(lease.holder.session, lease.cwd, sessions, false),
      isQueued: false,
      card: jobCard.forActive(job, now, lastKnown, stopOf(lease)),
      isEnding: isEnding(lease),
    };
    (isBenchmark(lease) ? benchmark : running).push(row);
  }
  for (const job of shown.queue) {
    const { lease } = job;
    const alsoRunning = runningSessions.has(lease.holder.session);
    const card = jobCard.forQueued(
      job,
      now,
      benchmarkAhead && !isBenchmark(lease),
      lastKnown,
      stopOf(lease),
      othersActive,
    );
    const row = {
      id: lease.id,
      command: lease.label,
      session: strip(lease.holder.session, lease.cwd, sessions, alsoRunning),
      isQueued: true,
      card,
      isEnding: isEnding(lease),
    };
    if (isBenchmark(lease)) {
      benchmark.push(row);
      benchmarkAhead = benchmarkAhead || !row.isEnding;
    } else {
      waiting.push(row);
    }
  }
  benchmark.push(...finished(state, shown, sessions, now));

  /** @type {JobGroup[]} */
  const groups = [];
  if (benchmark.length > 0) {
    groups.push({ kind: "benchmark", heading: "Benchmark", jobs: benchmark });
  }
  if (running.length > 0) {
    groups.push({ kind: "running", heading: counted("Running", running), jobs: running });
  }
  if (waiting.length > 0) {
    groups.push({ kind: "waiting", heading: counted("Waiting", waiting), jobs: waiting });
  }

  const phase = phaseOf(benchmark, shown);
  return {
    header: header(status, alive(running), alive(waiting), phase),
    groups,
    idle: idleLine([...sessions.values()], state),
    recent: groups.length === 0 ? recentLines(state, now) : [],
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
 * An active benchmark runs alone; a queued one either drains the machine or, once it is at
 * the head and drained, measures the background load.
 *
 * @param {JobRow[]} rows
 * @param {ShownJobs} shown
 * @returns {BenchmarkPhase | null}
 */
function phaseOf(rows, shown) {
  const live = rows.filter((row) => !row.isEnding);
  const [first] = live;
  if (first === undefined) {
    return null;
  }
  const activeIds = new Set(shown.active.map((job) => job.lease.id));
  if (live.some((row) => activeIds.has(row.id))) {
    return "running-alone";
  }
  const verdict = shown.queue.find((job) => job.lease.id === first.id)?.verdict;
  switch (verdict?.benchmark?.phase) {
    case "measuring":
    case "noisy":
      return "waiting-for-quiet";
    default:
      return "draining";
  }
}

/**
 * @param {UiSession[]} sessions
 * @param {UiState | null} state
 * @returns {IdleLine | null}
 */
function idleLine(sessions, state) {
  if (state === null) {
    return null;
  }
  const busy = new Set([
    ...state.active.map((job) => job.lease.holder.session),
    ...state.queue.map((job) => job.lease.holder.session),
  ]);
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
 * Benchmarks that finished on their own in the last `FINISHED_HOLD_MS`, from allotd's recent
 * runs, so the card survives a restart of the shell. A killed one already showed Stopped.
 *
 * @param {UiState | null} state
 * @param {ShownJobs} shown
 * @param {Map<string, UiSession>} sessions
 * @param {number} now
 * @returns {JobRow[]}
 */
function finished(state, shown, sessions, now) {
  const live = new Set([
    ...shown.active.map((job) => job.lease.id),
    ...shown.queue.map((job) => job.lease.id),
  ]);
  /** @type {JobRow[]} */
  const rows = [];
  for (const run of state?.recent ?? []) {
    const { noise, lease_id: id } = run;
    if (
      noise === null ||
      id === null ||
      (run.end_cause !== null && run.end_cause !== "released") ||
      now - run.ended_at_ms >= FINISHED_HOLD_MS ||
      live.has(id)
    ) {
      continue;
    }
    rows.push({
      id,
      command: run.command,
      session: strip(run.session ?? "", run.cwd, sessions, false),
      isQueued: false,
      card: jobCard.forFinished(run, noise),
      isEnding: true,
    });
  }
  return rows;
}

/**
 * @param {UiState | null} state
 * @param {number} now
 * @returns {RecentLine[]}
 */
function recentLines(state, now) {
  return (state?.recent ?? []).map((run) => {
    const ran = format.duration((run.ended_at_ms - run.started_at_ms) / 1000);
    const ago = format.ago(Math.floor(Math.max(0, now - run.ended_at_ms) / 1000));
    return {
      command: run.command,
      shortId: run.session === null ? "" : format.shortId(run.session),
      meta: `${ran} · ${ago}`,
    };
  });
}

/**
 * While every card in a group shows Stopped or Removed, the heading keeps their number until
 * they collapse, as the handoff prototype does.
 *
 * @param {string} label
 * @param {JobRow[]} rows
 * @returns {string}
 */
function counted(label, rows) {
  const live = alive(rows);
  return `${label} · ${live > 0 ? live : rows.length}`;
}

/**
 * @param {JobRow[]} rows
 * @returns {number}
 */
function alive(rows) {
  return rows.filter((row) => !row.isEnding).length;
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
 * @param {string} key
 * @param {string} jobCwd
 * @param {Map<string, UiSession>} sessions
 * @param {boolean} alsoRunning
 * @returns {SessionStrip}
 */
function strip(key, jobCwd, sessions, alsoRunning) {
  const session = sessions.get(key);
  const cwd = session?.cwd || jobCwd;
  let usage = "";
  if (alsoRunning) {
    usage = "also running";
  } else if (session !== undefined) {
    usage = format.usage(session.usage);
  }
  return {
    shortId: format.shortId(key),
    project: format.project(cwd),
    projectPath: cwd,
    usage,
  };
}
