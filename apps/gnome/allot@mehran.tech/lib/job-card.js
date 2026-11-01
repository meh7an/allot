import * as benchmarkStatus from "./benchmark-status.js";
import * as format from "./format.js";

/** @import { ActiveJob, Lease, NoiseReport, QueuedJob, RecentRun } from "../contract.g.mjs" */
/** @import { BenchmarkStatus, BenchmarkSteps, DetailIcon } from "./benchmark-status.js" */
/** @import { StopView } from "./job-stops.js" */

/**
 * What the 2 px edge bar along a card's bottom shows.
 *
 * - `running`: green, elapsed against the estimate.
 * - `over`: amber and full, past the estimate.
 * - `waiting`: amber, waited against the expected wait.
 * - `waiting-dashed`: amber dashes across the whole card; the wait has no estimate.
 * - `frozen`: gray and still; a stopped job, or last known state while the service is down.
 * - `none`: no bar and no track; a removed request.
 *
 * @typedef {"running" | "over" | "waiting" | "waiting-dashed" | "frozen" | "none"} EdgeBarTone
 */

/**
 * The control at a card's right.
 *
 * - `none`: a spacer keeps the layout.
 * - `kill`: the Kill icon button of a running job.
 * - `confirm`: the red Kill? button that confirms a kill.
 * - `remove`: the Remove text button of a queued request.
 *
 * @typedef {"none" | "kill" | "confirm" | "remove"} CardAction
 */

/**
 * The job row of a card at one moment (handoff section 4).
 *
 * @typedef {object} JobCard
 * @property {string} tile
 *   Two-letter initials of the process, or `#N` for a queued job.
 * @property {boolean} isQueued
 *   The tile is the dashed queue position.
 * @property {string | null} exePath
 *   The process executable, whose app icon replaces the initials when it has one; `null` for
 *   queued jobs and unknown paths.
 * @property {string} chip
 * @property {string} chipTooltip
 * @property {boolean} isBenchmark
 *   The chip is the solid purple Benchmark chip.
 * @property {string} strong
 *   The bold start of the detail line (`Stopped`, `Removed`, `Finished`); empty otherwise.
 * @property {string} detail
 *   The detail line's left part, after `strong`.
 * @property {string} detailTooltip
 *   The detail's full text for its accessible name, or empty.
 * @property {boolean} detailIsDanger
 *   The detail reads in red: the Kill? confirm, or a refusal.
 * @property {string} time
 *   The detail line's right part.
 * @property {boolean} timeIsWarning
 *   The time reads in amber: past the estimate, or waiting.
 * @property {number} barFraction
 *   How much of the edge bar is filled, from 0 to 1.
 * @property {EdgeBarTone} barTone
 * @property {CardAction} action
 * @property {string} actionName
 *   The action's accessible name, for example `Kill cargo`.
 * @property {boolean} isEnded
 *   The job has ended (stopped, removed, finished, ended by allotd): the tile, command and chip
 *   show at 50%.
 * @property {boolean} isSending
 *   A Kill or Remove went to allotd and its answer is not back: `Stopping…` or `Removing…`.
 * @property {boolean} isStruck
 *   The command is struck through: stopped or removed, not finished.
 * @property {DetailIcon} icon
 *   The detail line's leading icon; with one, the line wraps instead of truncating.
 * @property {BenchmarkSteps | null} steps
 *   A benchmark's Drain / Quiet / Run stepper, or `null` for none.
 * @property {string} announcement
 *   What a screen reader announces when a stop ends or is refused, for example
 *   `Stopped cargo · Claude was notified`; empty otherwise.
 */

/**
 * @typedef {object} Chip
 * @property {string} label
 * @property {string} tooltip
 * @property {boolean} isBenchmark
 */

// A queued job this close to its start reads "starts next".
const STARTS_NEXT_SECS = 60;

/**
 * @param {ActiveJob} job The running job.
 * @param {number} nowMs The moment to show, in the daemon's clock.
 * @param {boolean} lastKnown The job comes from the last state allotd served before it went
 *   away.
 * @param {StopView | null} stop A Kill under way or done in this popup, or `null`; ignored for
 *   last known state.
 * @returns {JobCard}
 */
export function forActive(job, nowMs, lastKnown, stop) {
  const card = activeCard(job, nowMs, lastKnown, stop);
  if (!card.isBenchmark) {
    return card;
  }
  const { lease } = job;
  const elapsed = seconds(nowMs - (lease.granted_at_ms ?? lease.requested_at_ms));
  const status = benchmarkStatus.forActive(job, elapsed);
  return benchmark(card, status, lastKnown, stop?.kind === "kill" ? stop : null);
}

/**
 * A benchmark that finished on its own: the green check, the run time and its background
 * noise, every step complete, and no action.
 *
 * @param {RecentRun} run
 * @param {NoiseReport} noise
 * @returns {JobCard}
 */
export function forFinished(run, noise) {
  const status = benchmarkStatus.forFinished(run, noise);
  return {
    ...base(format.initials(firstWord(run.command)), false, null, BENCHMARK_CHIP),
    detail: status.text,
    barTone: "none",
    icon: status.icon,
    steps: status.steps,
  };
}

/**
 * @param {QueuedJob} job The waiting request.
 * @param {number} nowMs The moment to show, in the daemon's clock.
 * @param {boolean} afterBenchmark A benchmark runs or waits ahead of it.
 * @param {boolean} lastKnown The job comes from the last state allotd served before it went
 *   away.
 * @param {StopView | null} stop A Remove under way or done in this popup, or `null`; ignored
 *   for last known state.
 * @param {boolean} othersActive Another lease is active, which a queued benchmark drains.
 * @returns {JobCard}
 */
export function forQueued(job, nowMs, afterBenchmark, lastKnown, stop, othersActive) {
  const card = queuedCard(job, nowMs, afterBenchmark, lastKnown, stop);
  if (!card.isBenchmark) {
    return card;
  }
  return benchmark(
    card,
    benchmarkStatus.forQueued(job, othersActive),
    lastKnown,
    stop?.kind === "remove" ? stop : null,
  );
}

/** @type {Chip} */
const BENCHMARK_CHIP = { label: "Benchmark", tooltip: "Benchmark", isBenchmark: true };

/**
 * A card with every field at its quiet default.
 *
 * @param {string} tile
 * @param {boolean} isQueued
 * @param {string | null} exePath
 * @param {Chip} chip
 * @returns {JobCard}
 */
function base(tile, isQueued, exePath, chip) {
  return {
    tile,
    isQueued,
    exePath,
    chip: chip.label,
    chipTooltip: chip.tooltip,
    isBenchmark: chip.isBenchmark,
    strong: "",
    detail: "",
    detailTooltip: "",
    detailIsDanger: false,
    time: "",
    timeIsWarning: false,
    barFraction: 0,
    barTone: "running",
    action: "none",
    actionName: "",
    isEnded: false,
    isSending: false,
    isStruck: false,
    icon: "none",
    steps: null,
    announcement: "",
  };
}

/**
 * @param {ActiveJob} job
 * @param {number} nowMs
 * @param {boolean} lastKnown
 * @param {StopView | null} stop
 * @returns {JobCard}
 */
function activeCard(job, nowMs, lastKnown, stop) {
  const { lease, process } = job;
  const started = lease.granted_at_ms ?? lease.requested_at_ms;
  const elapsed = seconds(nowMs - started);
  const estimate = lease.profile.est_secs;
  const over = estimate > 0 && elapsed > estimate;
  const progress = estimate > 0 ? fraction(elapsed, estimate) : 0;
  const time =
    estimate > 0
      ? `${format.duration(elapsed)} / ~${format.duration(estimate)}`
      : format.duration(elapsed);
  const prefix = process === null ? "" : `${process.name} ${process.pid} · `;
  const tile = format.initials(process?.name ?? firstWord(lease.label));
  const card = base(tile, false, process?.exe ?? null, chip(lease));

  if (lastKnown) {
    return { ...card, detail: `${prefix}was at ${time}`, barFraction: progress, barTone: "frozen" };
  }

  const detail = prefix + format.reservation(lease.profile);
  const name = process?.name ?? lease.label;
  Object.assign(card, {
    detail,
    detailTooltip: detail,
    time: over ? `${format.duration(elapsed - estimate)} over` : time,
    timeIsWarning: over,
    barFraction: over ? 1 : progress,
    barTone: over ? "over" : "running",
    action: "kill",
    actionName: `Kill ${name}`,
  });
  const outside = endedOutside(card, stop, started, estimate, nowMs);
  if (outside !== null) {
    return outside;
  }
  if (stop === null || stop.kind !== "kill") {
    return card;
  }

  switch (stop.phase) {
    case "confirming": {
      const confirm = `Stops ${process?.name ?? "the command"} and its child processes`;
      return {
        ...card,
        detail: confirm,
        detailTooltip: confirm,
        detailIsDanger: true,
        time: `${stop.secondsLeft}s`,
        timeIsWarning: false,
        action: "confirm",
        actionName: `Confirm kill ${name}`,
      };
    }
    case "sending":
      return {
        ...card,
        detail: process === null ? "Stopping…" : `Stopping ${process.name}…`,
        detailTooltip: "",
        time: "",
        timeIsWarning: false,
        action: "none",
        actionName: "",
        isSending: true,
      };
    case "failed": {
      const failed = `Not stopped: ${stop.message}`;
      return {
        ...card,
        detail: failed,
        detailTooltip: failed,
        detailIsDanger: true,
        announcement: `${name} not stopped: ${stop.message}`,
      };
    }
    case "ended": {
      const ran = seconds((stop.endedAtMs ?? nowMs) - started);
      return {
        ...card,
        strong: "Stopped",
        detail: notified(stop.message),
        detailTooltip: "",
        time: `at ${format.duration(ran)}`,
        timeIsWarning: false,
        barFraction: estimate > 0 ? fraction(ran, estimate) : 0,
        barTone: "frozen",
        action: "none",
        actionName: "",
        isEnded: true,
        isStruck: true,
        announcement: `Stopped ${name}${notified(stop.message)}`,
      };
    }
  }
}

/**
 * @param {QueuedJob} job
 * @param {number} nowMs
 * @param {boolean} afterBenchmark
 * @param {boolean} lastKnown
 * @param {StopView | null} stop
 * @returns {JobCard}
 */
function queuedCard(job, nowMs, afterBenchmark, lastKnown, stop) {
  const { lease } = job;
  const waited = seconds(nowMs - lease.requested_at_ms);
  const left = job.verdict.wait_estimate_secs;

  let time;
  let filled = 1;
  /** @type {EdgeBarTone} */
  let tone = "waiting";
  if (afterBenchmark) {
    time = `${format.duration(waited)} · after benchmark`;
    tone = "waiting-dashed";
  } else if (job.position === 1 && (job.verdict.fits || (left ?? Infinity) <= STARTS_NEXT_SECS)) {
    time = left === null ? "starts next" : `starts next · ~${format.minutes(left)}`;
    filled = left === null ? 1 : fraction(waited, waited + left);
  } else if (left !== null) {
    const total = waited + left;
    time = `${format.duration(waited)} / ~${format.minutes(total)}`;
    filled = fraction(waited, total);
  } else {
    time = `${format.duration(waited)} · depends on queue`;
    tone = "waiting-dashed";
  }

  const card = {
    ...base(`#${job.position}`, true, null, chip(lease)),
    detail: format.reservation(lease.profile),
    time,
    timeIsWarning: true,
    barFraction: filled,
    barTone: lastKnown ? /** @type {const} */ ("frozen") : tone,
    action: lastKnown ? /** @type {const} */ ("none") : /** @type {const} */ ("remove"),
    actionName: lastKnown ? "" : `Remove ${lease.label} from queue`,
  };
  if (lastKnown || stop === null || stop.kind !== "remove") {
    return card;
  }

  switch (stop.phase) {
    case "sending":
      return { ...card, detail: "Removing…", action: "none", actionName: "", isSending: true };
    case "failed": {
      const failed = `Not removed: ${stop.message}`;
      return {
        ...card,
        detail: failed,
        detailTooltip: failed,
        detailIsDanger: true,
        announcement: `${lease.label} not removed: ${stop.message}`,
      };
    }
    case "ended":
      return {
        ...card,
        strong: "Removed",
        detail: notified(stop.message),
        time: "",
        timeIsWarning: false,
        barFraction: 0,
        barTone: "none",
        action: "none",
        actionName: "",
        isEnded: true,
        isStruck: true,
        announcement: `Removed ${lease.label}${notified(stop.message)}`,
      };
    case "confirming":
      return card;
  }
}

/**
 * A benchmark shows its progress on the stepper instead of an edge bar, and its status line
 * instead of the reservation. While a stop is under way the stepper stays and the line is the
 * stop's; once stopped or removed, and in last known state, only the plain line is left,
 * without a time, as in the prototype.
 *
 * @param {JobCard} card
 * @param {BenchmarkStatus} status
 * @param {boolean} lastKnown
 * @param {StopView | null} stop
 * @returns {JobCard}
 */
function benchmark(card, status, lastKnown, stop) {
  const plain = { ...card, barFraction: 0, barTone: /** @type {const} */ ("none") };
  if (plain.isEnded) {
    return { ...plain, time: "" };
  }
  if (lastKnown) {
    return plain;
  }
  if (stop !== null) {
    return {
      ...plain,
      time: stop.phase === "confirming" ? plain.time : "",
      timeIsWarning: false,
      steps: status.steps,
    };
  }
  return {
    ...plain,
    detail: status.text,
    detailTooltip: "",
    time: "",
    timeIsWarning: false,
    icon: status.icon,
    steps: status.steps,
  };
}

/**
 * A job that ended without this popup stopping it, held like Stopped: Finished with the green
 * check and its run time, not struck through; a kill from elsewhere reads as a stop; allotd's
 * own end gives its reason.
 *
 * @param {JobCard} card
 * @param {StopView | null} stop
 * @param {number} started
 * @param {number} estimate
 * @param {number} nowMs
 * @returns {JobCard | null} The held card, or `null` when `stop` is no such end.
 */
function endedOutside(card, stop, started, estimate, nowMs) {
  if (
    stop === null ||
    stop.phase !== "ended" ||
    !["finished", "killed-outside", "ended-by-allotd"].includes(stop.kind)
  ) {
    return null;
  }
  const ran = seconds((stop.endedAtMs ?? nowMs) - started);
  const ended = {
    ...card,
    detailTooltip: "",
    time: `at ${format.duration(ran)}`,
    timeIsWarning: false,
    barFraction: estimate > 0 ? fraction(ran, estimate) : 0,
    barTone: /** @type {const} */ ("frozen"),
    action: /** @type {const} */ ("none"),
    actionName: "",
    isEnded: true,
  };
  switch (stop.kind) {
    case "finished":
      return {
        ...ended,
        strong: "Finished",
        detail: ` · ${format.duration(ran)}`,
        time: "",
        icon: "check",
      };
    case "killed-outside":
      return {
        ...ended,
        strong: "Stopped outside the popup",
        detail: notified(""),
        isStruck: true,
      };
    default:
      return {
        ...ended,
        strong: "Ended by allotd",
        detail: stop.message === "" ? "" : ` · ${stop.message}`,
        detailTooltip: stop.message,
      };
  }
}

/**
 * @param {string} note
 * @returns {string}
 */
function notified(note) {
  return note === "" ? " · Claude was notified" : ` · ${note} · Claude was notified`;
}

/**
 * @param {Lease} lease
 * @returns {Chip}
 */
function chip(lease) {
  if (lease.kind === "benchmark_exclusive") {
    return BENCHMARK_CHIP;
  }
  /** @type {Record<string, string>} */
  const labels = { cpu_heavy: "CPU heavy", gpu_heavy: "GPU heavy", mixed: "Mixed", light: "Light" };
  if (lease.class === null) {
    return { label: "Custom", tooltip: "Custom reservation", isBenchmark: false };
  }
  const label = labels[lease.class] ?? "Custom";
  return { label, tooltip: label, isBenchmark: false };
}

/**
 * @param {number} milliseconds
 * @returns {number}
 */
function seconds(milliseconds) {
  return Math.max(0, Math.floor(milliseconds / 1000));
}

/**
 * @param {number} part
 * @param {number} whole
 * @returns {number}
 */
function fraction(part, whole) {
  return whole <= 0 ? 1 : Math.min(1, Math.max(0, part / whole));
}

/**
 * @param {string} command
 * @returns {string}
 */
function firstWord(command) {
  return command.replace(/^ +/, "").split(" ")[0] ?? "";
}
