import assert from "node:assert/strict";
import { test } from "node:test";

import { PopupModel } from "../allot@mehran.tech/lib/popup-model.js";
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
