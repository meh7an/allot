import assert from "node:assert/strict";
import { test } from "node:test";

import { StateClock } from "../allot@mehran.tech/lib/state-clock.js";
import { NOW_MS, scenario } from "./handoff.mjs";

test("without a live state there is no time", () => {
  assert.equal(new StateClock(() => 0).nowMs, null);
});

test("time runs on from the state's moment and stops for last known state", () => {
  let local = 5000;
  const clock = new StateClock(() => local);

  clock.received(scenario("one").state);
  local += 2500;
  assert.equal(clock.nowMs, NOW_MS + 2500);

  clock.received(null);
  assert.equal(clock.nowMs, null);
});
