import assert from "node:assert/strict";
import { test } from "node:test";

import { BusClient, BusRefusal } from "../allot@mehran.tech/lib/bus-client.js";
import { CancelledError, TimeoutError } from "../allot@mehran.tech/lib/results.js";
import { restartService } from "../allot@mehran.tech/lib/service-launcher.js";
import { FakeBus, FakeTimers, fixtureText } from "./support.mjs";

/** @import { BusCall } from "../allot@mehran.tech/lib/bus-client.js" */

/** @param {(call: BusCall) => string | null | Promise<string | null>} handler */
function client(handler) {
  const fake = new FakeBus(handler);
  return { daemon: new BusClient(fake.bus()), fake };
}

/** @param {string} name A fixture under `contract/fixtures`. */
function fixture(name) {
  return JSON.parse(fixtureText(`${name}.json`));
}

test("a poll while allotd is not on the bus answers stopped without a call", async () => {
  const { daemon, fake } = client(() => {
    throw new Error("no call expected");
  });
  fake.owner = false;

  const polled = await daemon.poll();

  assert.equal(polled.status, "stopped");
  assert.deepEqual(fake.calls, []);
});

test("a poll calls GetState and decodes the popup's state", async () => {
  const { daemon, fake } = client(() => fixtureText("ui-state-idle.json"));

  const polled = await daemon.poll();

  assert.equal(polled.status, "running");
  assert.equal(polled.version, polled.state?.service.version);
  assert.deepEqual(fake.calls, [
    { method: "GetState", signature: null, args: [], timeoutMs: 3000 },
  ]);
});

test("a poll before the first watcher answer still calls", async () => {
  const { daemon, fake } = client(() => fixtureText("ui-state-idle.json"));
  fake.owner = null;

  assert.equal((await daemon.poll()).status, "running");
});

test("a poll before allotd's first sample is starting, and another refusal incompatible", async () => {
  const unavailable = client(() => {
    throw new BusRefusal("Unavailable", "no machine sample yet");
  });
  assert.equal((await unavailable.daemon.poll()).status, "starting");

  const denied = client(() => {
    throw new BusRefusal("AccessDenied", "allotd serves only user 1000");
  });
  const polled = await denied.daemon.poll();
  assert.equal(polled.status, "incompatible");
  assert.match(polled.detail ?? "", /AccessDenied/);
});

test("a poll that reaches nothing is stopped, and a cancel is passed on", async () => {
  const gone = client(() => {
    throw new Error("The name is not activatable");
  });
  assert.equal((await gone.daemon.poll()).status, "stopped");

  const cancelled = client(() => {
    throw new CancelledError();
  });
  await assert.rejects(cancelled.daemon.poll(), CancelledError);
});

test("a kill sends the lease as an int64 with the longer timeout", async () => {
  const { daemon, fake } = client(() => fixtureText("kill-response-tree.json"));

  const result = await daemon.kill(7);

  assert.ok(result.ended);
  assert.deepEqual(fake.calls, [
    { method: "Kill", signature: "(x)", args: [7], timeoutMs: 10_000 },
  ]);
});

test("a refused kill reads as the reason the card shows", async () => {
  const conflict = client(() => {
    throw new BusRefusal("Conflict", "lease 7 was not killed: its process tree is protected");
  });
  assert.deepEqual(await conflict.daemon.kill(7), {
    ended: false,
    text: "its process tree is protected",
    endedAtMs: null,
  });

  const slow = client(() => {
    throw new TimeoutError();
  });
  assert.equal((await slow.daemon.kill(7)).text, "the service did not answer within 10 s");

  const gone = client(() => {
    throw new Error("The name is not activatable");
  });
  assert.equal((await gone.daemon.remove(7)).text, "the service is not running");
});

test("a remove decodes the remove-response fixture", async () => {
  const { daemon, fake } = client(() => fixtureText("remove-response.json"));

  assert.ok((await daemon.remove(3)).ended);
  assert.equal(fake.calls[0]?.method, "Remove");
});

test("Stop service asks for the stop record, and a restart does not", async () => {
  const { daemon, fake } = client(() => null);

  assert.equal(await daemon.shutdown(), null);
  assert.equal(await daemon.shutdown({ record: false }), null);

  assert.deepEqual(
    fake.calls.map((call) => [call.method, call.signature, call.args]),
    [
      ["Shutdown", "(b)", [true]],
      ["Shutdown", "(b)", [false]],
    ],
  );
});

test("the settings state fixture decodes from GetSettings", async () => {
  const { daemon, fake } = client(() => fixtureText("settings-state.json"));

  const reply = await daemon.settings();

  assert.ok(reply.ok);
  assert.equal(reply.value.revision, "69fd99abe4f654c7");
  assert.deepEqual(fake.calls[0], {
    method: "GetSettings",
    signature: null,
    args: [],
    timeoutMs: 3000,
  });
});

test("a patch goes to ChangeSettings as the settings-patch fixture's JSON", async () => {
  const patch = fixture("settings-patch");
  const { daemon, fake } = client(() => fixtureText("settings-state.json"));

  assert.ok((await daemon.changeSettings(patch)).ok);

  const [call] = fake.calls;
  assert.equal(call?.method, "ChangeSettings");
  assert.equal(call?.signature, "(s)");
  assert.deepEqual(JSON.parse(String(call?.args[0])), patch);
});

test("preview and classify send their requests as JSON", async () => {
  const classify = fixture("classify-request");
  const answers = {
    PreviewSettings: fixtureText("fit-preview.json"),
    Classify: fixtureText("command-test.json"),
  };
  const { daemon, fake } = client((call) => answers[call.method] ?? null);

  assert.ok((await daemon.previewSettings([])).ok);
  assert.ok((await daemon.classify(classify)).ok);

  assert.deepEqual(JSON.parse(String(fake.calls[0]?.args[0])), { changes: [] });
  assert.deepEqual(JSON.parse(String(fake.calls[1]?.args[0])), classify);
});

test("a conflict refusal carries the file's revision and problems", async () => {
  const refusal = fixtureText("settings-refusal-conflict.json");
  const { daemon } = client(() => {
    throw new BusRefusal("Conflict", refusal);
  });

  const reply = await daemon.changeSettings(fixture("settings-patch"));

  assert.ok(!reply.ok);
  assert.equal(reply.failure.kind, "conflict");
  assert.equal(reply.failure.revision, JSON.parse(refusal).revision);
});

test("a refusal whose message is not JSON keeps its text", async () => {
  const { daemon } = client(() => {
    throw new BusRefusal("Held", "config.toml is held by another program");
  });

  const reply = await daemon.settings();

  assert.ok(!reply.ok);
  assert.equal(reply.failure.kind, "held");
  assert.equal(reply.failure.message, "config.toml is held by another program");
});

test("a settings call that reaches nothing is unreachable", async () => {
  const { daemon } = client(() => {
    throw new Error("The name is not activatable");
  });

  const reply = await daemon.settings();

  assert.ok(!reply.ok);
  assert.equal(reply.failure.kind, "unreachable");
});

test("the version comes from the Version property, and is unknown while allotd is away", async () => {
  const { daemon, fake } = client(() => null);
  fake.version = async () => "0.6.0";
  assert.equal(await daemon.healthVersion(), "0.6.0");

  fake.version = async () => {
    throw new Error("The name is not activatable");
  };
  assert.equal(await daemon.healthVersion(), null);

  fake.owner = false;
  assert.equal(await daemon.healthVersion(), null);
});

test("Restart Service over the bus needs no port to follow", async () => {
  const { daemon, fake } = client(() => null);
  fake.owner = false;
  const launcher = {
    restartThroughUnit: async () => false,
    start: async () => null,
  };

  assert.equal(await restartService(daemon, launcher, new FakeTimers(), 47823), null);
  assert.deepEqual(
    fake.calls.map((call) => [call.method, call.args]),
    [["Shutdown", [false]]],
  );
});

const GDBUS_TEXT =
  "GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown: The name tech.mehran.Allot was not " +
  "provided by any .service files";

test("Stop service names why allotd did not stop, never the GDBus text", async () => {
  const gone = client(() => {
    throw new Error(GDBUS_TEXT);
  });
  assert.equal(await gone.daemon.shutdown(), "the service is not on the session bus");

  const slow = client(() => {
    throw new TimeoutError();
  });
  assert.equal(await slow.daemon.shutdown(), "the service did not answer within 3 s");

  const busy = client(() => {
    throw new BusRefusal("Unavailable", "the service has just started; try again");
  });
  assert.equal(await busy.daemon.shutdown(), "the service has just started; try again");

  const cancelled = client(() => {
    throw new CancelledError();
  });
  await assert.rejects(cancelled.daemon.shutdown(), CancelledError);
});

test("a poll that failed inside allotd is retried as starting, not incompatible", async () => {
  const failed = client(() => {
    throw new BusRefusal("Failed", "see the log");
  });

  const polled = await failed.daemon.poll();

  assert.equal(polled.status, "starting");
  assert.doesNotMatch(polled.detail ?? "", /Failed|GDBus/);
});

/** @param {boolean} alive */
function clientWithOffBus(alive) {
  const fake = new FakeBus(() => {
    throw new Error(GDBUS_TEXT);
  });
  return { daemon: new BusClient(fake.bus(), async () => alive), fake };
}

test("no owner and a live allotd polls as off the bus, with no call", async () => {
  const { daemon, fake } = clientWithOffBus(true);
  fake.owner = false;

  const polled = await daemon.poll();

  assert.equal(polled.status, "off_bus");
  assert.equal(polled.detail, "the service runs without the session bus");
  assert.deepEqual(fake.calls, []);
});

test("a call that finds no allotd on the bus asks the probe, settings included", async () => {
  const alive = clientWithOffBus(true);
  assert.equal((await alive.daemon.poll()).status, "off_bus");
  const reply = await alive.daemon.settings();
  assert.ok(!reply.ok);
  assert.equal(reply.failure.kind, "off_bus");

  const dead = clientWithOffBus(false);
  assert.equal((await dead.daemon.poll()).status, "stopped");
  const refused = await dead.daemon.settings();
  assert.ok(!refused.ok);
  assert.equal(refused.failure.kind, "unreachable");
});

test("a timeout is a busy allotd, never one off the bus", async () => {
  const fake = new FakeBus(() => {
    throw new TimeoutError();
  });
  const daemon = new BusClient(fake.bus(), async () => true);

  assert.equal((await daemon.poll()).status, "stopped");
  const reply = await daemon.settings();
  assert.ok(!reply.ok);
  assert.equal(reply.failure.kind, "unreachable");
});

test("the GDBus text does not reach a poll's detail or a settings failure", async () => {
  const { daemon } = client(() => {
    throw new Error(GDBUS_TEXT);
  });

  const polled = await daemon.poll();
  const reply = await daemon.settings();

  assert.equal(polled.status, "stopped");
  assert.equal(polled.detail, "the service is not on the session bus");
  assert.ok(!reply.ok);
  assert.equal(reply.failure.kind, "unreachable");
  assert.equal(reply.failure.message, "the service is not on the session bus");

  const slow = client(() => {
    throw new TimeoutError();
  });
  assert.equal((await slow.daemon.poll()).detail, "the service did not answer within 3 s");
});
