import assert from "node:assert/strict";
import { test } from "node:test";

import { BusClient } from "../allot@mehran.tech/lib/bus-client.js";
import * as format from "../allot@mehran.tech/lib/format.js";
import { DaemonPoller, START_TIMEOUT_MS } from "../allot@mehran.tech/lib/poller.js";
import { FakeBus, FakeTimers, fixtureText, settle } from "./support.mjs";

/**
 * A poller whose launcher answers `problem`, with a clock the test moves.
 *
 * @param {{ up: boolean }} state
 * @param {string | null} [problem]
 */
function starter(state, problem = null) {
  const body = fixtureText("ui-state-idle.json");
  const transport = new FakeBus(() => {
    if (!state.up) {
      throw new Error("The name is not activatable");
    }
    return body;
  });
  const daemon = new BusClient(transport.bus());
  const clock = { nowMs: 0, starts: 0 };
  const poller = new DaemonPoller(daemon, new FakeTimers(), {
    launcher: {
      start: async () => {
        clock.starts += 1;
        return problem;
      },
    },
    now: () => clock.nowMs,
  });
  return { poller, clock, transport };
}

test("Start Service reports starting until allotd answers", async () => {
  const state = { up: false };
  const { poller, clock } = starter(state);

  assert.equal(await poller.startService(), null);
  assert.equal(clock.starts, 1);
  await poller.pollOnce();
  assert.equal(poller.latest?.status, "starting");
  assert.equal(poller.startProblem, null);

  state.up = true;
  await poller.pollOnce();
  assert.equal(poller.latest?.status, "running");
});

test("Start Service gives up after its window and says why", async () => {
  const { poller, clock } = starter({ up: false });
  await poller.startService();

  clock.nowMs = START_TIMEOUT_MS;
  await poller.pollOnce();

  assert.equal(poller.latest?.status, "stopped");
  assert.equal(poller.startProblem, "allotd did not answer within 10 s; see logs/allotd.log");
  assert.equal(poller.latest?.detail, poller.startProblem);

  await poller.pollOnce();
  assert.equal(poller.latest?.status, "stopped");
});

test("a failed launch stays stopped and keeps its problem until allotd answers", async () => {
  const state = { up: false };
  const { poller } = starter(state, "/x/allotd is missing; run allot install");

  assert.equal(await poller.startService(), "/x/allotd is missing; run allot install");
  await poller.pollOnce();
  assert.equal(poller.latest?.status, "stopped");
  assert.equal(poller.startProblem, "/x/allotd is missing; run allot install");

  state.up = true;
  await poller.pollOnce();
  assert.equal(poller.startProblem, null);
});

test("Start Service wakes the poll loop", async () => {
  const { poller, transport } = starter({ up: true });
  const loop = poller.start();
  await settle();
  const before = transport.calls.length;

  await poller.startService();
  await settle();

  assert.ok(transport.calls.length > before);
  poller.stop();
  await loop;
});

test("up to two decimals drops trailing zeros", () => {
  assert.equal(format.upToTwoDecimals(1.2057), "1.21");
  assert.equal(format.upToTwoDecimals(1.8), "1.8");
  assert.equal(format.upToTwoDecimals(2), "2");
});
