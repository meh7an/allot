import assert from "node:assert/strict";
import { test } from "node:test";

import * as jobCard from "../allot@mehran.tech/lib/job-card.js";
import { panelSnapshot } from "../allot@mehran.tech/lib/snapshot.js";
import { NOW_MS, decodeState, finishedBenchmark, stateJson } from "./handoff.mjs";

/** @import { ActiveJob, QueuedJob, UiState } from "../allot@mehran.tech/contract.g.mjs" */

/**
 * The state with a change made to its JSON first.
 *
 * @param {import("./handoff.mjs").StateOptions} options
 * @param {(json: any) => void} [edit]
 * @returns {UiState}
 */
function state(options, edit = () => {}) {
  const json = stateJson(options);
  edit(json);
  return decodeState(json);
}

/**
 * @param {UiState} from
 * @returns {ActiveJob}
 */
function firstActive(from) {
  const [job] = from.active;
  assert.ok(job);
  return job;
}

/**
 * @param {UiState} from
 * @returns {QueuedJob}
 */
function firstQueued(from) {
  const [job] = from.queue;
  assert.ok(job);
  return job;
}

test("the running bar is elapsed over the estimate and ticks with the clock", () => {
  const job = firstActive(state({ run: ["cargo"] }));

  const now = jobCard.forActive(job, NOW_MS, false, null);
  const later = jobCard.forActive(job, NOW_MS + 4000, false, null);

  assert.ok(Math.abs(now.barFraction - 0.62) < 0.001);
  assert.equal(now.time, "3:06 / ~5:00");
  assert.equal(later.time, "3:10 / ~5:00");
});

test("without an estimate only the elapsed time shows", () => {
  const job = firstActive(
    state({ run: ["cargo"] }, (json) => {
      json.active[0].lease.profile.est_secs = 0;
    }),
  );

  const card = jobCard.forActive(job, NOW_MS, false, null);

  assert.equal(card.time, "3:06");
  assert.equal(card.barFraction, 0);
  assert.equal(card.barTone, "running");
});

test("without a process the command names the job", () => {
  const job = firstActive(
    state({ run: ["cargo"] }, (json) => {
      json.active[0].process = null;
    }),
  );

  const card = jobCard.forActive(job, NOW_MS, false, null);

  assert.equal(card.detail, "8 cores · 4 GB");
  assert.equal(card.tile, "CA");
  assert.equal(card.exePath, null);
  assert.equal(card.actionName, "Kill cargo build --release");
});

test("Kill? and Stopping name the process, a refusal says why", () => {
  const job = firstActive(state({ run: ["cargo"] }));
  /** @param {import("../allot@mehran.tech/lib/job-stops.js").StopPhase} phase */
  const stop = (phase) => ({
    kind: /** @type {const} */ ("kill"),
    phase,
    secondsLeft: 2,
    message: "it is protected",
    endedAtMs: null,
  });

  assert.equal(jobCard.forActive(job, NOW_MS, false, stop("confirming")).time, "2s");
  const sending = jobCard.forActive(job, NOW_MS, false, stop("sending"));
  assert.equal(sending.detail, "Stopping cargo…");
  assert.equal(sending.action, "none");
  assert.ok(sending.isSending);
  const failed = jobCard.forActive(job, NOW_MS, false, stop("failed"));
  assert.equal(failed.detail, "Not stopped: it is protected");
  assert.ok(failed.detailIsDanger);
  assert.equal(failed.action, "kill");
});

test("a job that ended outside the popup reads how", () => {
  const job = firstActive(state({ run: ["cargo"] }));
  /** @param {import("../allot@mehran.tech/lib/job-stops.js").StopKind} kind */
  const ended = (kind) => ({
    kind,
    phase: /** @type {const} */ ("ended"),
    secondsLeft: 0,
    message: "idle for 180s",
    endedAtMs: NOW_MS,
  });

  const finished = jobCard.forActive(job, NOW_MS, false, ended("finished"));
  assert.equal(finished.strong, "Finished");
  assert.equal(finished.detail, " · 3:06");
  assert.equal(finished.icon, "check");
  assert.equal(finished.isStruck, false);
  const outside = jobCard.forActive(job, NOW_MS, false, ended("killed-outside"));
  assert.equal(outside.strong, "Stopped outside the popup");
  assert.ok(outside.isStruck);
  const byAllotd = jobCard.forActive(job, NOW_MS, false, ended("ended-by-allotd"));
  assert.equal(byAllotd.strong, "Ended by allotd");
  assert.equal(byAllotd.detail, " · idle for 180s");
});

test("the head of the queue starts next when it fits or has a minute left", () => {
  const close = firstQueued(
    state({ wait: ["npm"] }, (json) => {
      json.queue[0].verdict.wait_estimate_secs = 50;
    }),
  );
  const fits = firstQueued(
    state({ wait: ["ctest"] }, (json) => {
      json.queue[0].verdict.fits = true;
    }),
  );

  /** @param {QueuedJob} job */
  const time = (job) => jobCard.forQueued(job, NOW_MS, false, false, null, false).time;

  assert.equal(time(close), "starts next · ~1 min");
  assert.equal(time(fits), "starts next");
});

test("the second in line with a minute left does not start next", () => {
  const queue = state({ wait: ["ctest", "npm"] }, (json) => {
    json.queue[1].verdict.wait_estimate_secs = 50;
  }).queue;
  const second = queue[1];
  assert.ok(second);

  const card = jobCard.forQueued(second, NOW_MS, false, false, null, false);

  assert.equal(card.time, "0:42 / ~2 min");
});

test("a last known queued job has no Remove and a frozen bar", () => {
  const job = firstQueued(state({ wait: ["npm"] }));

  const card = jobCard.forQueued(job, NOW_MS, false, true, null, false);

  assert.equal(card.action, "none");
  assert.equal(card.actionName, "");
  assert.equal(card.barTone, "frozen");
});

test("a drained benchmark that does not fit waits for room", () => {
  const job = firstQueued(
    state({ bench: "quiet" }, (json) => {
      json.queue[0].verdict.benchmark = null;
    }),
  );

  assert.equal(
    jobCard.forQueued(job, NOW_MS, false, false, null, false).detail,
    "Waiting for room · not enough free cores",
  );
  assert.equal(
    jobCard.forQueued(job, NOW_MS, false, false, null, true).detail,
    "Draining · waiting for running jobs to finish",
  );
});

test("a running benchmark past its estimate says how far over", () => {
  const job = firstActive(state({ bench: "run" }));

  const card = jobCard.forActive(job, NOW_MS + 100_000, false, null);

  assert.equal(card.detail, "Running alone · 0:28 over");
});

test("a finished benchmark that was not quiet says so", () => {
  const run = finishedBenchmark(false);
  assert.ok(run.noise);

  assert.equal(
    jobCard.forFinished(run, run.noise).detail,
    "Finished in 2:04 · not quiet at start · background noise avg 1.8%, peak 6.2% CPU",
  );
});

test("a last known benchmark keeps only its plain line", () => {
  const card = jobCard.forActive(firstActive(state({ bench: "run" })), NOW_MS, true, null);

  assert.equal(card.detail, "cargo 30412 · was at 0:48 / ~2:00");
  assert.equal(card.steps, null);
  assert.equal(card.barTone, "none");
});

test("recent runs show only while nothing runs or waits", () => {
  const busy = state({ run: ["cargo"], recent: true });

  assert.deepEqual(panelSnapshot("running", busy).recent, []);
  assert.equal(panelSnapshot("running", state({ recent: true })).recent.length, 2);
  assert.ok(panelSnapshot("running", state({})).showNothing);
});

test("a recent run without a session has no id", () => {
  const empty = state({ recent: true }, (json) => {
    json.recent[0].session = null;
  });

  assert.equal(panelSnapshot("running", empty).recent[0]?.shortId, "");
});
