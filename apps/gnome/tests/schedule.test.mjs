import assert from "node:assert/strict";
import { test } from "node:test";

import { PollSchedule } from "../allot@mehran.tech/lib/schedule.js";

test("running follows the popup", () => {
  const schedule = new PollSchedule();

  assert.equal(schedule.next(true, "running"), 1000);
  assert.equal(schedule.next(false, "running"), 5000);
});

test("starting polls twice a second", () => {
  assert.equal(new PollSchedule().next(false, "starting"), 500);
});

test("down doubles up to the closed cap", () => {
  for (const status of /** @type {const} */ (["stopped", "incompatible"])) {
    const schedule = new PollSchedule();

    const delays = Array.from({ length: 7 }, () => schedule.next(false, status));

    assert.deepEqual(delays, [1000, 2000, 4000, 8000, 16000, 30000, 30000], status);
  }
});

test("an open popup caps the retry at five seconds", () => {
  const schedule = new PollSchedule();

  const delays = Array.from({ length: 4 }, () => schedule.next(true, "stopped"));

  assert.deepEqual(delays, [1000, 2000, 4000, 5000]);
});

test("running or reset starts the backoff over", () => {
  const schedule = new PollSchedule();
  schedule.next(false, "stopped");
  schedule.next(false, "stopped");

  schedule.next(false, "running");
  assert.equal(schedule.next(false, "stopped"), 1000);

  schedule.reset();
  assert.equal(schedule.next(false, "stopped"), 1000);
});
