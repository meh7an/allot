import assert from "node:assert/strict";
import { test } from "node:test";

import { fixtureText } from "../../gnome/tests/support.mjs";
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

/** @param {string} name A fixture under `contract/fixtures`. */
function fixture(name) {
  return JSON.parse(fixtureText(`${name}.json`));
}

test("Stop service sends the panel's stop record request", async () => {
  const { daemon, transport } = client(() => ({ status: 202, body: "" }));

  assert.equal(await daemon.shutdown(), null);

  const [request] = transport.requests;
  assert.equal(request?.method, "POST");
  assert.equal(request?.url, "http://127.0.0.1:1/shutdown");
  assert.deepEqual(JSON.parse(request?.body ?? ""), fixture("shutdown-request"));
});

test("the settings state fixture decodes with every setting's value", async () => {
  const body = fixtureText("settings-state.json");
  const { daemon, transport } = client(() => ({ status: 200, body }));

  const reply = await daemon.settings();

  assert.ok(reply.ok);
  assert.equal(reply.value.revision, "69fd99abe4f654c7");
  assert.deepEqual(reply.value.pending_restart, ["daemon.port"]);
  assert.equal(reply.value.listening_port, 47823);
  assert.equal(reply.value.file_error, null);
  assert.equal(reply.value.values.length, 23);
  assert.deepEqual(transport.requests[0], {
    method: "GET",
    url: "http://127.0.0.1:1/settings",
    token: "t",
    timeoutSecs: 3,
  });
});

test("a patch is sent as the settings-patch fixture encodes it", async () => {
  const patch = fixture("settings-patch");
  const body = fixtureText("settings-state.json");
  const { daemon, transport } = client(() => ({ status: 200, body }));

  const reply = await daemon.changeSettings(patch);

  assert.ok(reply.ok);
  const [request] = transport.requests;
  assert.equal(request?.method, "PATCH");
  assert.equal(request?.url, "http://127.0.0.1:1/settings");
  assert.deepEqual(JSON.parse(request?.body ?? ""), patch);
});

test("a command test sends the classify-request fixture and decodes the answer", async () => {
  const classify = fixture("classify-request");
  const body = fixtureText("command-test.json");
  const { daemon, transport } = client(() => ({ status: 200, body }));

  const reply = await daemon.classify(classify);

  assert.ok(reply.ok);
  assert.deepEqual(
    reply.value.commands.map((command) => command.kind),
    ["heavy", "print_only", "not_heavy"],
  );
  assert.equal(reply.value.lease?.class, "cpu_heavy");
  assert.equal(transport.requests[0]?.url, "http://127.0.0.1:1/classify");
  assert.deepEqual(JSON.parse(transport.requests[0]?.body ?? ""), classify);
});

test("a preview sends the unsaved changes and decodes what fits", async () => {
  const body = fixtureText("fit-preview.json");
  const { daemon, transport } = client(() => ({ status: 200, body }));
  /** @type {import("../allot@mehran.tech/contract.g.mjs").Change[]} */
  const changes = [{ kind: "set_number", key: "admission.headroom_pct", value: 30 }];

  const reply = await daemon.previewSettings(changes);

  assert.ok(reply.ok);
  assert.equal(reply.value.kept_free.total_cores, 16);
  assert.equal(transport.requests[0]?.url, "http://127.0.0.1:1/settings/preview");
  assert.deepEqual(JSON.parse(transport.requests[0]?.body ?? ""), { changes });
});

test("refusals keep allotd's message, problems and revision", async () => {
  const cases = [
    { status: 409, file: "settings-refusal-conflict", kind: "conflict" },
    { status: 422, file: "settings-refusal-invalid", kind: "invalid" },
  ];
  for (const { status, file, kind } of cases) {
    const refusal = fixture(file);
    const { daemon } = client(() => ({ status, body: JSON.stringify(refusal) }));

    const reply = await daemon.settings();

    assert.ok(!reply.ok);
    assert.equal(reply.failure.kind, kind);
    assert.equal(reply.failure.message, refusal.message);
    assert.deepEqual(reply.failure.problems, refusal.problems);
    assert.equal(reply.failure.revision, refusal.revision ?? null);
  }
});

test("each other answer has its own kind", async () => {
  const held = JSON.stringify({ message: "config.toml is open", problems: [] });
  const cases = [
    { response: { status: 423, body: held }, kind: "held", message: "config.toml is open" },
    { response: { status: 503, body: "" }, kind: "unavailable", message: "the service answered 503" },
    {
      response: { status: 404, body: "" },
      kind: "outdated",
      message: "the service answered 404",
    },
    { response: { status: 500, body: "internal error\n" }, kind: "failed", message: "internal error" },
    { response: { status: 401, body: "" }, kind: "key", message: "the service refused the token" },
  ];
  for (const { response, kind, message } of cases) {
    const { daemon } = client(() => response);

    const reply = await daemon.settings();

    assert.ok(!reply.ok);
    assert.equal(reply.failure.kind, kind);
    assert.equal(reply.failure.message, message);
  }
});

test("an answer that is not a settings state fails", async () => {
  const { daemon } = client(() => ({ status: 200, body: '{"spec": 1}' }));

  const reply = await daemon.settings();

  assert.ok(!reply.ok);
  assert.equal(reply.failure.kind, "failed");
});

test("no token and no answer are told apart, and a cancel rejects", async () => {
  const noToken = client(() => ({ status: 200, body: "" }), null);
  const down = client(() => {
    throw new Error("Connection refused");
  });
  const cancelled = client(() => {
    throw new CancelledError();
  });

  const missing = await noToken.daemon.settings();
  const unreachable = await down.daemon.settings();

  assert.ok(!missing.ok && missing.failure.kind === "key");
  assert.equal(noToken.transport.requests.length, 0);
  assert.ok(!unreachable.ok && unreachable.failure.kind === "unreachable");
  assert.equal(unreachable.failure.message, "Connection refused");
  await assert.rejects(cancelled.daemon.settings(), CancelledError);
});
