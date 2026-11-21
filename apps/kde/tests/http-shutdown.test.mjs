import assert from "node:assert/strict";
import { test } from "node:test";

import { FakeTimers, fixtureText, settle } from "../../gnome/tests/support.mjs";
import { DaemonPoller } from "../../gnome/allot@mehran.tech/lib/poller.js";
import { CancelledError } from "../../gnome/allot@mehran.tech/lib/results.js";
import { DaemonClient } from "../src/http-client.js";
import { FakeTransport } from "./support.mjs";

/** @import { HttpRequest, HttpResponse } from "../src/http-client.js" */

/**
 * @param {(request: HttpRequest) => HttpResponse} handler
 * @param {string | null} [token]
 */
function client(handler, token = "t") {
  const transport = new FakeTransport(handler);
  const daemon = new DaemonClient({ port: 1, send: transport.send, readToken: async () => token });
  return { daemon, transport };
}

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
