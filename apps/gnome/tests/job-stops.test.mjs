import assert from "node:assert/strict";
import { test } from "node:test";

import { JobStops } from "../allot@mehran.tech/lib/job-stops.js";
import { refused } from "../allot@mehran.tech/lib/stop-result.js";
import { NOW_MS, decodeState, leaseId, stateJson, without } from "./handoff.mjs";

const CARGO = leaseId("cargo");
const PY = leaseId("py");
const NPM = leaseId("npm");
const ENDED = { ended: true, text: "", endedAtMs: NOW_MS };

/** A local clock the tests move by hand. */
class TestClock {
  ms = 1000;
  now = () => this.ms;

  /** @param {number} ms */
  advance(ms) {
    this.ms += ms;
  }
}

/** @param {import("./handoff.mjs").StateOptions} options */
function state(options) {
  return decodeState(stateJson(options));
}

/** Stops that have seen the `several` state: cargo and py run, npm waits. */
function seeded(clock = new TestClock()) {
  const stops = new JobStops(clock.now);
  stops.observe(state({ run: ["cargo", "py"], wait: ["npm"] }));
  return stops;
}

/**
 * Kill pressed twice past the guard, so the request is under way.
 *
 * @param {TestClock} clock
 * @param {JobStops} stops
 */
function sendKill(clock, stops) {
  stops.pressKill(CARGO);
  clock.advance(1000);
  assert.equal(stops.pressKill(CARGO), "send");
}

test("the first press confirms and counts down from four", () => {
  const clock = new TestClock();
  const stops = seeded(clock);

  assert.equal(stops.pressKill(CARGO), "confirming");
  assert.deepEqual(stops.views.get(CARGO), {
    kind: "kill",
    phase: "confirming",
    secondsLeft: 4,
    message: "",
    endedAtMs: null,
  });
  assert.ok(stops.isConfirming);
  clock.advance(1500);
  assert.equal(stops.views.get(CARGO)?.secondsLeft, 3);
  clock.advance(2400);
  assert.equal(stops.views.get(CARGO)?.secondsLeft, 1);
});

test("a confirm drops after four seconds", () => {
  const clock = new TestClock();
  const stops = seeded(clock);
  stops.pressKill(CARGO);

  clock.advance(4000);

  assert.ok(stops.expire());
  assert.equal(stops.views.size, 0);
});

test("a second press within 300 ms is ignored, after it the kill is sent", () => {
  const clock = new TestClock();
  const stops = seeded(clock);
  stops.pressKill(CARGO);

  clock.advance(300);
  assert.equal(stops.pressKill(CARGO), "ignored");
  assert.equal(stops.views.get(CARGO)?.phase, "confirming");

  clock.advance(1);
  assert.equal(stops.pressKill(CARGO), "send");
  assert.equal(stops.views.get(CARGO)?.phase, "sending");
  assert.equal(stops.pressKill(CARGO), "ignored");
});

test("an ended stop holds four seconds with its note", () => {
  const clock = new TestClock();
  const stops = seeded(clock);
  sendKill(clock, stops);

  stops.finished(CARGO, { ended: true, text: "1 process still running", endedAtMs: NOW_MS });

  assert.deepEqual(stops.views.get(CARGO), {
    kind: "kill",
    phase: "ended",
    secondsLeft: 0,
    message: "1 process still running",
    endedAtMs: NOW_MS,
  });
  assert.equal(stops.pressKill(CARGO), "ignored");
  clock.advance(3999);
  assert.equal(stops.expire(), false);
  clock.advance(1);
  assert.ok(stops.expire());
});

test("a refusal holds six seconds, and a press confirms again", () => {
  const clock = new TestClock();
  const stops = seeded(clock);
  sendKill(clock, stops);

  stops.finished(CARGO, refused("it is protected"));

  assert.equal(stops.views.get(CARGO)?.phase, "failed");
  assert.equal(stops.views.get(CARGO)?.message, "it is protected");
  clock.advance(5000);
  assert.equal(stops.expire(), false);
  assert.equal(stops.pressKill(CARGO), "confirming");
});

test("a refused remove goes after six seconds", () => {
  const clock = new TestClock();
  const stops = seeded(clock);
  assert.ok(stops.pressRemove(NPM));
  stops.finished(NPM, refused("the service is not running"));

  clock.advance(6000);

  assert.ok(stops.expire());
});

test("remove sends at once, and only once", () => {
  const stops = seeded();

  assert.ok(stops.pressRemove(NPM));
  assert.equal(stops.views.get(NPM)?.phase, "sending");
  assert.equal(stops.pressRemove(NPM), false);
});

test("presses of the wrong kind or of an unknown job are ignored", () => {
  const stops = seeded();

  assert.equal(stops.pressKill(NPM), "ignored");
  assert.equal(stops.pressRemove(CARGO), false);
  assert.equal(stops.pressKill(999), "ignored");
  assert.equal(stops.views.size, 0);
});

test("cancel takes back only a confirm, cancelConfirms every one", () => {
  const stops = seeded();
  stops.pressKill(CARGO);
  stops.pressKill(PY);
  stops.pressRemove(NPM);

  assert.equal(stops.cancel(NPM), false);
  assert.ok(stops.cancel(CARGO));
  assert.ok(stops.cancelConfirms());
  assert.equal(stops.isConfirming, false);
  assert.deepEqual([...stops.views.keys()], [NPM]);
});

test("a confirm is dropped once its job is gone", () => {
  const stops = seeded();
  stops.pressKill(CARGO);

  stops.observe(state({ run: ["py"], wait: ["npm"] }));

  assert.equal(stops.views.size, 0);
});

test("a stop under way keeps its card at its place", () => {
  const clock = new TestClock();
  const stops = seeded(clock);
  sendKill(clock, stops);
  stops.pressRemove(NPM);

  const after = state({ run: ["py"] });
  stops.observe(after);
  const jobs = stops.merge(after);

  assert.deepEqual(
    jobs.active.map((job) => job.lease.id),
    [CARGO, PY],
  );
  assert.deepEqual(
    jobs.queue.map((job) => job.lease.id),
    [NPM],
  );
});

test("the next change is the next countdown second or hold end", () => {
  const clock = new TestClock();
  const stops = seeded(clock);
  assert.equal(stops.nextChangeMs, null);

  stops.pressKill(CARGO);
  clock.advance(400);
  assert.equal(stops.nextChangeMs, 600);

  clock.advance(600);
  stops.pressKill(CARGO);
  assert.equal(stops.nextChangeMs, null);
  stops.finished(CARGO, ENDED);
  clock.advance(1000);
  assert.equal(stops.nextChangeMs, 3000);
});

test("a job that ends elsewhere holds its card with how it ended", () => {
  const cases = [
    ["released", "finished", ""],
    ["killed", "killed-outside", ""],
    ["expired", "ended-by-allotd", "the holder is gone"],
  ];
  for (const [cause, kind, message] of cases) {
    const stops = seeded();
    const before = state({ run: ["cargo", "py"], wait: ["npm"] });
    const after = without(before, CARGO, /** @type {"released"} */ (cause));

    stops.observe(after);

    assert.equal(stops.views.get(CARGO)?.kind, kind, cause);
    assert.equal(stops.views.get(CARGO)?.phase, "ended");
    assert.equal(stops.views.get(CARGO)?.message, message);
    assert.equal(stops.merge(after).active[0]?.lease.id, CARGO);
  }
});

test("a job gone without an ended entry, or a removal, is not held", () => {
  const stops = seeded();
  const before = state({ run: ["cargo", "py"], wait: ["npm"] });

  stops.observe(without(before, CARGO));
  assert.equal(stops.views.size, 0);

  const removed = seeded();
  removed.observe(without(before, CARGO, "removed"));
  assert.equal(removed.views.size, 0);
});

test("an ended benchmark is not held, since recent runs carry it", () => {
  const stops = new JobStops(new TestClock().now);
  const running = state({ bench: "run" });
  stops.observe(running);

  stops.observe(without(running, leaseId("bench"), "released"));

  assert.equal(stops.views.size, 0);
});
