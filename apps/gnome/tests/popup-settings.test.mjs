import assert from "node:assert/strict";
import { test } from "node:test";

import { PopupModel } from "../allot@mehran.tech/lib/popup-model.js";
import { refused } from "../allot@mehran.tech/lib/stop-result.js";
import { leaseId, scenario } from "./handoff.mjs";

/** @import { PollResult } from "../allot@mehran.tech/lib/client.js" */

/**
 * @param {PollResult["status"]} status
 * @param {PollResult["state"]} [state]
 * @returns {PollResult}
 */
function poll(status, state = null) {
  return { status, state, version: null, detail: null };
}

test("the card and the banner follow the poll, and meters count as a change", () => {
  const model = new PopupModel();
  const { state } = scenario("several");
  model.apply(poll("running", state), state);

  assert.equal(model.resources.isVisible, true);
  assert.equal(model.banner, null);

  const busier = scenario("several").state;
  busier.meters.cpu.usage_pct = 20;
  assert.deepEqual(model.apply(poll("running", busier), busier), {
    snapshot: true,
    indicator: false,
  });

  model.apply(poll("stopped"), busier);
  assert.equal(model.resources.isVisible, false);
  assert.equal(model.banner?.button, "start");
  assert.equal(model.banner?.caption.startsWith("Last known · "), true);
});

test("the banner says why a start failed, and Start Service shows Starting", () => {
  const model = new PopupModel();
  const { state } = scenario("one");
  model.apply(poll("stopped"), state, "/x/allotd is missing; run allot install");
  assert.equal(model.banner?.problem, "/x/allotd is missing; run allot install");

  model.noteStarting();

  assert.equal(model.status, "starting");
  assert.equal(model.banner?.button, "starting");
  assert.equal(model.banner?.problem, "");
});

test("the caption's time of day comes from the shell", () => {
  const model = new PopupModel();
  model.timeOfDay = (ms) => `at ${ms}`;
  const { state } = scenario("one");

  model.apply(poll("stopped"), state);

  assert.equal(model.banner?.caption, `Last known · at ${state.generated_at_ms}`);
});

test("settings change the card and go to be saved; an equal change does neither", () => {
  const model = new PopupModel();
  const { state } = scenario("several");
  model.apply(poll("running", state), state);
  /** @type {boolean[]} */
  const saved = [];
  model.onSettingsChanged = (settings) => saved.push(settings.resources_expanded);

  assert.deepEqual(model.toggleResources(), { snapshot: true, indicator: false });
  assert.equal(model.resources.isExpanded, true);
  assert.equal(model.resources.summary, "");
  assert.deepEqual(saved, [true]);

  assert.deepEqual(
    model.updateSettings((settings) => {
      settings.resources_expanded = true;
    }),
    { snapshot: false, indicator: false },
  );
  assert.deepEqual(saved, [true]);
});

test("settings read at startup show without being saved again", () => {
  const model = new PopupModel();
  const { state } = scenario("several");
  model.apply(poll("running", state), state);
  model.onSettingsChanged = () => assert.fail("saved");

  model.loadSettings({ ...model.settings, resources_expanded: true });

  assert.equal(model.resources.isExpanded, true);
});

test("the first-run card shows until Done, and opens the popup once", () => {
  const model = new PopupModel();
  const { state } = scenario("first-run");
  /** @type {boolean[]} */
  const saved = [];
  model.onSettingsChanged = (settings) => saved.push(settings.first_run_shown);
  model.apply(poll("running", state), state);

  assert.deepEqual(model.snapshot.welcome, { doNotDisturb: false });
  assert.equal(model.wantsFirstOpen, true);
  assert.deepEqual(saved, []);
  model.noteFirstOpened();
  assert.equal(model.wantsFirstOpen, false);
  assert.deepEqual(saved, [true]);

  model.setDoNotDisturb(true);
  assert.deepEqual(model.snapshot.welcome, { doNotDisturb: true });

  model.finishFirstRun();
  assert.equal(model.snapshot.welcome, null);
  assert.equal(model.wantsFirstOpen, false);
});

test("a session having connected means no first-run card and no opening", () => {
  const model = new PopupModel();
  const { state } = scenario("one");
  model.apply(poll("running", state), state);

  assert.equal(model.snapshot.welcome, null);
  assert.equal(model.wantsFirstOpen, false);
  assert.equal(model.settings.first_run_shown, false);
});

test("the first-run card needs live data", () => {
  const model = new PopupModel();
  const { state } = scenario("first-run");
  model.apply(poll("stopped"), state);

  assert.equal(model.snapshot.welcome, null);
  assert.equal(model.wantsFirstOpen, false);
});

test("a Kill tells the rules before it goes out", () => {
  const clock = { nowMs: 1000 };
  const model = new PopupModel(() => clock.nowMs);
  const { state } = scenario("one");
  model.apply(poll("running", state), state);
  /** @type {string[]} */
  const calls = [];
  model.onKillSent = (lease) => calls.push(`noted ${lease}`);
  model.stopper = {
    kill: async (lease) => {
      calls.push(`killed ${lease}`);
      return refused("no");
    },
    remove: async () => refused("no"),
  };
  const lease = leaseId("cargo");

  model.pressKill(lease);
  assert.deepEqual(calls, []);

  clock.nowMs += 500;
  model.pressKill(lease);
  assert.deepEqual(calls, [`noted ${lease}`, `killed ${lease}`]);
});
