import assert from "node:assert/strict";
import { test } from "node:test";

import { PopupModel } from "../allot@mehran.tech/lib/popup-model.js";
import { NOW_MS, leaseId, scenario, without } from "./handoff.mjs";
import { fixtureState } from "./support.mjs";

/** @import { PollResult } from "../allot@mehran.tech/lib/client.js" */

/**
 * @param {PollResult["status"]} status
 * @param {PollResult["state"]} [state]
 * @returns {PollResult}
 */
function poll(status, state = null) {
  return { status, state, version: null, detail: null };
}

test("a new model shows starting until the first poll", () => {
  const model = new PopupModel();

  assert.equal(model.snapshot.header.summary, "Starting service…");
  assert.equal(model.indicator.icon, "down");
  assert.equal(model.mode, "light-dismiss");
});

test("apply shows the poll and says what changed", () => {
  const model = new PopupModel();
  const state = fixtureState("linux");

  const change = model.apply(poll("running", state), state);

  assert.deepEqual(change, { snapshot: true, indicator: true });
  assert.equal(model.snapshot.header.summary, "2 running · 1 waiting");
  assert.equal(model.indicator.icon, "queued");
});

test("an equal poll changes nothing", () => {
  const model = new PopupModel();
  const state = fixtureState("linux");
  model.apply(poll("running", state), state);

  const change = model.apply(poll("running", fixtureState("linux")), state);

  assert.deepEqual(change, { snapshot: false, indicator: false });
});

test("a stopped poll keeps the last known jobs dimmed", () => {
  const model = new PopupModel();
  const state = fixtureState("linux");
  model.apply(poll("running", state), state);

  model.apply(poll("stopped"), state);

  assert.equal(model.snapshot.isLastKnown, true);
  assert.equal(model.snapshot.groups.length, 2);
  assert.deepEqual(model.indicator, { icon: "down", summary: "Service stopped" });
});

test("stay open, then pin, then stay open off unpins", () => {
  const model = new PopupModel();

  model.toggleStayOpen();
  assert.equal(model.mode, "stay-open");
  model.togglePinned();
  assert.equal(model.mode, "pinned");
  assert.ok(model.isStayOpen && model.isPinned);
  model.toggleStayOpen();
  assert.equal(model.mode, "light-dismiss");
});

test("pin does nothing in light-dismiss mode", () => {
  const model = new PopupModel();

  model.togglePinned();

  assert.equal(model.mode, "light-dismiss");
});

test("unpin keeps stay open", () => {
  const model = new PopupModel();
  model.toggleStayOpen();
  model.togglePinned();

  model.togglePinned();

  assert.equal(model.mode, "stay-open");
});

test("reopening resets to light dismiss", () => {
  const model = new PopupModel();
  model.toggleStayOpen();
  model.togglePinned();

  model.resetForOpen();

  assert.equal(model.mode, "light-dismiss");
});

/** A local clock the tests move by hand. */
class TestClock {
  ms = 1000;
  now = () => this.ms;

  /** @param {number} ms */
  advance(ms) {
    this.ms += ms;
  }
}

/** Answers every stop as given and records the leases. */
class FakeStopper {
  /** @type {string[]} */
  calls = [];

  /** @param {import("../allot@mehran.tech/lib/stop-result.js").StopResult} answer */
  constructor(answer) {
    this.answer = answer;
  }

  /** @param {number} lease */
  kill = async (lease) => {
    this.calls.push(`kill ${lease}`);
    return this.answer;
  };

  /** @param {number} lease */
  remove = async (lease) => {
    this.calls.push(`remove ${lease}`);
    return this.answer;
  };
}

/**
 * A model that has seen the `several` scenario, with a stopper.
 *
 * @param {import("../allot@mehran.tech/lib/stop-result.js").StopResult} answer
 */
function stopModel(answer = { ended: true, text: "", endedAtMs: NOW_MS }) {
  const clock = new TestClock();
  const model = new PopupModel(clock.now);
  const stopper = new FakeStopper(answer);
  model.stopper = stopper;
  const state = scenario("several").state;
  model.apply(poll("running", state), state);
  return { clock, model, stopper, state };
}

/**
 * @param {PopupModel} model
 * @param {string} command
 */
function cardOf(model, command) {
  const rows = model.snapshot.groups.flatMap((group) => group.jobs);
  const row = rows.find((job) => job.command === command);
  assert.ok(row, command);
  return row.card;
}

test("kill twice sends, then holds Stopped past the lease until the hold ends", async () => {
  const { clock, model, stopper, state } = stopModel();
  /** @type {string[]} */
  const answered = [];
  model.onStopAnswered = (lease, kind) => answered.push(`${kind} ${lease}`);
  const cargo = leaseId("cargo");

  assert.equal(model.pressKill(cargo), null);
  assert.equal(cardOf(model, "cargo build --release").action, "confirm");
  assert.ok(model.isConfirming);
  clock.advance(500);
  const request = model.pressKill(cargo);
  assert.ok(request);
  assert.ok(cardOf(model, "cargo build --release").isSending);
  await request;

  assert.deepEqual(stopper.calls, [`kill ${cargo}`]);
  assert.deepEqual(answered, [`kill ${cargo}`]);
  assert.equal(cardOf(model, "cargo build --release").strong, "Stopped");
  assert.equal(model.snapshot.header.summary, "1 running · 1 waiting");

  const after = without(state, cargo, "killed");
  model.apply(poll("running", after), after);
  assert.equal(cardOf(model, "cargo build --release").strong, "Stopped");
  clock.advance(4000);
  model.refresh();
  assert.equal(
    model.snapshot.groups.flatMap((group) => group.jobs).some((job) => job.id === cargo),
    false,
  );
});

test("a refused remove says why", async () => {
  const { model } = stopModel({
    ended: false,
    text: "the job no longer exists",
    endedAtMs: null,
  });

  await model.pressRemove(leaseId("npm"));

  const card = cardOf(model, "npm run test:e2e");
  assert.equal(card.detail, "Not removed: the job no longer exists");
  assert.equal(card.action, "remove");
});

test("cancelConfirms and cancelConfirm turn Kill? back into Kill", () => {
  const { model } = stopModel();
  model.pressKill(leaseId("cargo"));
  model.pressKill(leaseId("py"));

  assert.ok(model.cancelConfirm(leaseId("py")));
  assert.equal(cardOf(model, "python train.py --epochs 20 --data ./shards").action, "kill");
  assert.ok(model.cancelConfirms());
  assert.equal(model.cancelConfirms(), false);
  assert.equal(cardOf(model, "cargo build --release").action, "kill");
});

test("the service going away cancels a confirm", () => {
  const { model, state } = stopModel();
  model.pressKill(leaseId("cargo"));

  model.apply(poll("stopped"), state);

  assert.equal(model.isConfirming, false);
  assert.equal(cardOf(model, "cargo build --release").action, "none");
});

test("refresh ticks times between polls", () => {
  const { clock, model } = stopModel();

  clock.advance(2000);
  const change = model.refresh();

  assert.deepEqual(change, { snapshot: true, indicator: false });
  assert.equal(cardOf(model, "cargo build --release").time, "3:08 / ~5:00");
  assert.deepEqual(model.refresh(), { snapshot: false, indicator: false });
});

test("a Removed card no longer makes the panel icon queued", async () => {
  const state = scenario("c-queued").state;
  const model = new PopupModel(new TestClock().now);
  model.stopper = new FakeStopper({ ended: true, text: "", endedAtMs: NOW_MS });
  model.apply(poll("running", state), state);
  assert.equal(model.indicator.icon, "queued");

  await model.pressRemove(leaseId("npm"));
  await model.pressRemove(leaseId("ctest"));

  assert.equal(model.indicator.icon, "idle");
});
