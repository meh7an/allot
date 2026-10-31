import assert from "node:assert/strict";
import { test } from "node:test";

import { CancelledError, DaemonClient } from "../allot@mehran.tech/lib/client.js";
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
