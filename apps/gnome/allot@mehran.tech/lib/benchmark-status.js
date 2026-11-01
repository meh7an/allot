import * as format from "./format.js";

/**
 * @import {
 *   ActiveJob,
 *   BenchmarkProgressMeasuring,
 *   NoiseReport,
 *   QueuedJob,
 *   RecentRun,
 * } from "../contract.g.mjs"
 */

/**
 * Where the Drain / Quiet / Run stepper stands; `done` when the run finished and every step is
 * complete.
 *
 * @typedef {"drain" | "quiet" | "run" | "done"} BenchmarkStep
 */

/**
 * The leading icon of a card's detail line.
 *
 * - `none`: no icon; the line truncates with an ellipsis.
 * - `timer`: the purple timer of a benchmark in progress; the line wraps.
 * - `check`: the green check of a finished benchmark; the line wraps.
 *
 * @typedef {"none" | "timer" | "check"} DetailIcon
 */

/**
 * The stepper under a benchmark's status line.
 *
 * @typedef {object} BenchmarkSteps
 * @property {BenchmarkStep} current
 *   The step in progress, or `done`.
 * @property {number} quietTicks
 *   Filled ticks of the Quiet step, out of `QUIET_TICKS`.
 * @property {number} runFraction
 *   How much of the Run step is filled, from 0 to 1.
 */

/**
 * A benchmark card's status line and stepper (handoff section 4, Benchmark).
 *
 * @typedef {object} BenchmarkStatus
 * @property {string} text
 * @property {DetailIcon} icon
 * @property {BenchmarkSteps} steps
 */

/** Ticks of the Quiet step, one per second of a 10 s quiet window. */
export const QUIET_TICKS = 10;

const NOT_QUIET = "not quiet at start · ";

/** @type {BenchmarkStatus} */
const DRAINING = {
  text: "Draining · waiting for running jobs to finish",
  icon: "timer",
  steps: { current: "drain", quietTicks: 0, runFraction: 0 },
};

/**
 * @param {ActiveJob} job The benchmark, granted and running alone.
 * @param {number} elapsed Seconds since it was granted.
 * @returns {BenchmarkStatus}
 */
export function forActive(job, elapsed) {
  const estimate = job.lease.profile.est_secs;
  let time = format.duration(elapsed);
  if (estimate > 0 && elapsed > estimate) {
    time = `${format.duration(elapsed - estimate)} over`;
  } else if (estimate > 0) {
    time = `${format.duration(elapsed)} / ~${format.duration(estimate)}`;
  }
  const forced = job.quiet_at_start === false ? NOT_QUIET : "";
  const fraction = estimate > 0 ? Math.min(1, Math.max(0, elapsed / estimate)) : 0;
  return {
    text: `Running alone · ${forced}${time}`,
    icon: "timer",
    steps: { current: "run", quietTicks: QUIET_TICKS, runFraction: fraction },
  };
}

/**
 * @param {QueuedJob} job The benchmark, still queued. Only the head of the queue carries
 *   progress.
 * @param {boolean} othersActive Another lease is still active, so a benchmark without progress
 *   waits for it to drain; otherwise it waits for room, with allotd's first reason.
 * @returns {BenchmarkStatus}
 */
export function forQueued(job, othersActive) {
  const progress = job.verdict.benchmark;
  switch (progress?.phase) {
    case "measuring": {
      const cpu = format.whole(progress.cpu_avg_pct);
      return {
        text:
          `Waiting for quiet · ${progress.measured_secs} / ${progress.needed_secs} s` +
          ` · background ${cpu}% CPU`,
        icon: "timer",
        steps: { current: "quiet", quietTicks: ticks(progress), runFraction: 0 },
      };
    }
    // The window is full but too busy; allotd grants it anyway once the longest wait ends.
    case "noisy": {
      const cpu = format.whole(progress.cpu_avg_pct);
      const limit = format.whole(progress.cpu_limit_pct);
      return {
        text:
          `Not quiet · background ${cpu}% CPU (limit ${limit}%)` +
          ` · starting anyway ${within(progress.grant_anyway_in_secs)}`,
        icon: "timer",
        steps: { current: "quiet", quietTicks: QUIET_TICKS, runFraction: 0 },
      };
    }
    case "draining":
      return DRAINING;
    case undefined:
      break;
  }
  if (othersActive) {
    return DRAINING;
  }
  // Drained, but its own reservation does not fit (seen live: 16 cores, 12.4 free).
  const [reason] = job.verdict.reasons;
  return {
    text: reason === undefined ? "Waiting for room" : `Waiting for room · ${reason}`,
    icon: "timer",
    steps: { current: "drain", quietTicks: 0, runFraction: 0 },
  };
}

/**
 * @param {RecentRun} run The finished benchmark run.
 * @param {NoiseReport} noise Its background load.
 * @returns {BenchmarkStatus}
 */
export function forFinished(run, noise) {
  const ran = Math.max(0, Math.floor((run.ended_at_ms - run.started_at_ms) / 1000));
  const forced = noise.quiet_at_start ? "" : NOT_QUIET;
  const average = format.oneDecimal(noise.avg_cpu_pct);
  const peak = format.oneDecimal(noise.peak_cpu_pct);
  return {
    text:
      `Finished in ${format.duration(ran)} · ${forced}background noise avg ` +
      `${average}%, peak ${peak}% CPU`,
    icon: "check",
    steps: { current: "done", quietTicks: QUIET_TICKS, runFraction: 1 },
  };
}

/**
 * The window may be configured longer or shorter than 10 s; the ticks show its share.
 *
 * @param {BenchmarkProgressMeasuring} measuring
 * @returns {number}
 */
function ticks(measuring) {
  if (measuring.needed_secs <= 0) {
    return QUIET_TICKS;
  }
  const share = Math.floor((measuring.measured_secs * QUIET_TICKS) / measuring.needed_secs);
  return Math.min(QUIET_TICKS, share);
}

/**
 * @param {number} seconds
 * @returns {string}
 */
function within(seconds) {
  return seconds === 0 ? "now" : `in ${format.duration(seconds)}`;
}
