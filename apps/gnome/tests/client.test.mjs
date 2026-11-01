import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CancelledError,
  DaemonClient,
  TimeoutError,
  conflictReason,
} from "../allot@mehran.tech/lib/client.js";
import { FakeTransport, fixtureText } from "./support.mjs";

/** @import { HttpRequest, HttpResponse } from "../allot@mehran.tech/lib/client.js" */

/**
 * A client of port 47999 whose token file holds `token.value` (`null`: no file).
 *
 * @param {(request: HttpRequest) => HttpResponse | Promise<HttpResponse>} handler
 * @param {{ value: string | null }} [token]
 */
function client(handler, token = { value: "t" }) {
  const transport = new FakeTransport(handler);
  const daemon = new DaemonClient({
    port: 47999,
    send: transport.send,
    readToken: async () => token.value,
  });
  return { daemon, transport };
}

/** @param {string} version */
function health(version) {
  return { status: 200, body: JSON.stringify({ version, pid: 1, uptime_secs: 5 }) };
}

test("a running state is decoded with the token", async () => {
  const body = fixtureText("ui-state-busy.json");
  const { daemon, transport } = client(() => ({ status: 200, body }), { value: "secret\n" });

  const result = await daemon.poll();

  assert.equal(result.status, "running");
  assert.equal(result.state?.active.length, 2);
  assert.equal(result.version, "0.2.0");
  assert.deepEqual(transport.requests, [
    { method: "GET", url: "http://127.0.0.1:47999/ui/state", token: "secret", timeoutSecs: 3 },
  ]);
});

test("a missing or empty token is stopped without a request", async () => {
  for (const value of [null, "  \n"]) {
    const { daemon, transport } = client(() => ({ status: 200, body: "" }), { value });

    const result = await daemon.poll();

    assert.equal(result.status, "stopped");
    assert.match(result.detail ?? "", /no token/);
    assert.equal(transport.requests.length, 0);
  }
});

test("an unreachable daemon is stopped", async () => {
  const { daemon } = client(() => {
    throw new Error("Could not connect: Connection refused");
  });

  const result = await daemon.poll();

  assert.equal(result.status, "stopped");
  assert.equal(result.detail, "Could not connect: Connection refused");
});

test("no first sample is starting", async () => {
  const { daemon } = client(() => ({ status: 503, body: "" }));

  assert.equal((await daemon.poll()).status, "starting");
});

test("an undecodable state is incompatible", async () => {
  for (const body of ['{"generated_at_ms": "soon"}', "not json"]) {
    const { daemon } = client(() => ({ status: 200, body }));

    assert.equal((await daemon.poll()).status, "incompatible", body);
  }
});

test("an old daemon without /ui/state is incompatible with its version", async () => {
  const { daemon, transport } = client((request) =>
    request.url.endsWith("/health") ? health("0.1.0") : { status: 404, body: "" },
  );

  const result = await daemon.poll();

  assert.equal(result.status, "incompatible");
  assert.equal(result.version, "0.1.0");
  assert.equal(transport.requests.at(-1)?.token, null);
});

test("not found without health is stopped", async () => {
  const { daemon } = client(() => ({ status: 404, body: "" }));

  assert.equal((await daemon.poll()).status, "stopped");
});

test("a refused token is read again once", async () => {
  const token = { value: "old" };
  const body = fixtureText("ui-state-idle.json");
  const { daemon, transport } = client(
    (request) => (request.token === "new" ? { status: 200, body } : { status: 401, body: "" }),
    token,
  );
  assert.equal((await daemon.poll()).status, "incompatible");

  token.value = "new";
  const result = await daemon.poll();

  assert.equal(result.status, "running");
  assert.equal(transport.requests.length, 4);
});

test("another status is incompatible", async () => {
  const { daemon } = client(() => ({ status: 500, body: "" }));

  const result = await daemon.poll();

  assert.equal(result.status, "incompatible");
  assert.equal(result.detail, "GET /ui/state answered 500");
});

test("a cancelled request rejects instead of reporting stopped", async () => {
  const { daemon } = client(() => {
    throw new CancelledError();
  });

  await assert.rejects(daemon.poll(), CancelledError);
});

/** The lease of a stop answer, ended at the handoff's moment. */
const ENDED_LEASE = {
  id: 7,
  holder: { session: "s", pid: 100, pid_start_time: 1 },
  kind: "normal",
  state: "expired",
  class: "cpu_heavy",
  profile: { cores: 4, ram_mb: 2048, gpu_pct: 0, vram_mb: 0, est_secs: 60 },
  label: "cargo build",
  cwd: "/home/user/work/app",
  requested_at_ms: 1_792_824_700_000,
  granted_at_ms: 1_792_824_710_000,
  ended_at_ms: 1_792_824_720_000,
  end_cause: "killed",
  expires_at_ms: 1_792_824_765_000,
};

/**
 * @param {number} failed
 * @param {string | null} untouched
 */
function killBody(failed = 0, untouched = null) {
  const process = { name: "cargo", pid: 18244, reason: "access denied" };
  return JSON.stringify({
    failed: Array.from({ length: failed }, () => process),
    lease: ENDED_LEASE,
    spared: [],
    terminated: [],
    untouched_reason: untouched,
  });
}

test("kill posts with the token and a ten second limit", async () => {
  const { daemon, transport } = client(() => ({ status: 200, body: killBody() }));

  const result = await daemon.kill(7);

  assert.deepEqual(result, { ended: true, text: "", endedAtMs: 1_792_824_720_000 });
  assert.deepEqual(transport.requests, [
    { method: "POST", url: "http://127.0.0.1:47999/leases/7/kill", token: "t", timeoutSecs: 10 },
  ]);
});

test("a kill notes what was left running", async () => {
  const cases = [
    [0, null, ""],
    [1, null, "1 process still running"],
    [3, null, "3 processes still running"],
    [0, "no bound process", "nothing was running"],
  ];
  for (const [failed, untouched, note] of cases) {
    const body = killBody(Number(failed), untouched === null ? null : String(untouched));
    const { daemon } = client(() => ({ status: 200, body }));

    assert.equal((await daemon.kill(7)).text, note);
  }
});

test("remove posts to the remove route", async () => {
  const body = JSON.stringify({ lease: { ...ENDED_LEASE, end_cause: "removed" } });
  const { daemon, transport } = client(() => ({ status: 200, body }));

  const result = await daemon.remove(7);

  assert.ok(result.ended);
  assert.equal(transport.requests[0]?.url, "http://127.0.0.1:47999/leases/7/remove");
});

test("refusals say why", async () => {
  const cases = [
    [
      409,
      "lease 7 was not killed: the bound process 1 (systemd) is protected",
      "the bound process 1 (systemd) is protected",
    ],
    [409, "lease 7 has already ended", "the job has already ended"],
    [409, "", "the service refused"],
    [404, "no such lease", "the job no longer exists"],
    [503, "", "the service has just started; try again"],
    [401, "", "the service refused the token"],
    [500, "boom", "the service answered 500"],
  ];
  for (const [status, body, reason] of cases) {
    const { daemon } = client(() => ({ status: Number(status), body: String(body) }));

    assert.deepEqual(await daemon.kill(7), { ended: false, text: reason, endedAtMs: null });
  }
  assert.equal(conflictReason(" lease 7 has already ended "), "the job has already ended");
});

test("an unreachable service says so", async () => {
  const timedOut = client(() => Promise.reject(new TimeoutError())).daemon;
  const refused = client(() => Promise.reject(new Error("connection refused"))).daemon;

  assert.equal((await timedOut.remove(7)).text, "the service did not answer within 10 s");
  assert.equal((await refused.remove(7)).text, "the service is not running");
});

test("an unreadable answer is refused", async () => {
  const { daemon } = client(() => ({ status: 200, body: "{}" }));

  assert.equal((await daemon.kill(7)).text, "the service's answer could not be read");
});

test("a missing token sends nothing", async () => {
  const { daemon, transport } = client(() => ({ status: 200, body: "" }), { value: null });

  assert.equal((await daemon.kill(7)).text, "the service's token file is missing");
  assert.deepEqual(transport.requests, []);
});

test("a cancelled stop rejects", async () => {
  const { daemon } = client(() => Promise.reject(new CancelledError()));

  await assert.rejects(daemon.kill(7), CancelledError);
});
