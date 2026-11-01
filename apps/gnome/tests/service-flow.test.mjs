import assert from "node:assert/strict";
import { test } from "node:test";

import { CancelledError, DaemonClient } from "../allot@mehran.tech/lib/client.js";
import * as format from "../allot@mehran.tech/lib/format.js";
import { DaemonPoller, START_TIMEOUT_MS } from "../allot@mehran.tech/lib/poller.js";
import { FakeTimers, FakeTransport, fixtureText, settle } from "./support.mjs";

/** @import { HttpRequest, HttpResponse } from "../allot@mehran.tech/lib/client.js" */

/**
 * @param {(request: HttpRequest) => HttpResponse} handler
 * @param {string | null} [token]
 */
function client(handler, token = "t") {
  const transport = new FakeTransport(handler);
  const daemon = new DaemonClient({ port: 1, send: transport.send, readToken: async () => token });
  return { daemon, transport };
}

/**
 * A poller whose launcher answers `problem`, with a clock the test moves.
 *
 * @param {{ up: boolean }} state
 * @param {string | null} [problem]
 */
function starter(state, problem = null) {
  const body = fixtureText("ui-state-idle.json");
  const { daemon, transport } = client(() => {
    if (!state.up) {
      throw new Error("Connection refused");
    }
    return { status: 200, body };
  });
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
  const before = transport.requests.length;

  await poller.startService();
  await settle();

  assert.ok(transport.requests.length > before);
  poller.stop();
  await loop;
});

test("Stop Service posts /shutdown and polls at once", async () => {
  const { daemon, transport } = client((request) =>
    request.url.endsWith("/shutdown")
      ? { status: 202, body: "" }
      : { status: 200, body: fixtureText("ui-state-idle.json") },
  );
  const poller = new DaemonPoller(daemon, new FakeTimers());
  const loop = poller.start();
  await settle();

  assert.equal(await poller.stopService(), null);
  await settle();

  const sent = transport.requests.map((request) => `${request.method} ${request.url}`);
  assert.ok(sent.includes("POST http://127.0.0.1:1/shutdown"));
  assert.equal(sent[sent.length - 1], "GET http://127.0.0.1:1/ui/state");
  poller.stop();
  await loop;
});

test("shutdown expects 202 and says why not", async () => {
  const accepted = client(() => ({ status: 202, body: "" }));
  assert.equal(await accepted.daemon.shutdown(), null);
  assert.equal(accepted.transport.requests[0]?.method, "POST");
  assert.equal(accepted.transport.requests[0]?.token, "t");

  const refused = client(() => ({ status: 500, body: "" }));
  assert.equal(await refused.daemon.shutdown(), "POST /shutdown answered 500");

  const noToken = client(() => ({ status: 202, body: "" }), null);
  assert.equal(await noToken.daemon.shutdown(), "no token in ~/.allot/token");

  const down = client(() => {
    throw new Error("Connection refused");
  });
  assert.equal(await down.daemon.shutdown(), "Connection refused");

  const cancelled = client(() => {
    throw new CancelledError();
  });
  await assert.rejects(cancelled.daemon.shutdown(), CancelledError);
});

test("up to two decimals drops trailing zeros", () => {
  assert.equal(format.upToTwoDecimals(1.2057), "1.21");
  assert.equal(format.upToTwoDecimals(1.8), "1.8");
  assert.equal(format.upToTwoDecimals(2), "2");
});
