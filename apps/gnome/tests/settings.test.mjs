import assert from "node:assert/strict";
import { test } from "node:test";

import { meterCount, notificationCount } from "../allot@mehran.tech/lib/menu-counts.js";
import {
  decodeSettings,
  defaultSettings,
  encodeSettings,
} from "../allot@mehran.tech/lib/settings.js";

test("the defaults match the handoff: every meter on, three notifications on", () => {
  const settings = defaultSettings();

  assert.equal(meterCount(settings.meters), "5 of 5");
  assert.equal(notificationCount(settings.notifications), "3 of 5");
  assert.equal(settings.notifications.service_stopped, true);
  assert.equal(settings.notifications.killed_outside_flyout, false);
  assert.equal(settings.resources_expanded, false);
  assert.equal(settings.first_run_done, false);
});

test("a missing, broken or non-object file gives the defaults", () => {
  assert.deepEqual(decodeSettings(null), defaultSettings());
  assert.deepEqual(decodeSettings("{ not json"), defaultSettings());
  assert.deepEqual(decodeSettings("[1]"), defaultSettings());
});

test("the other apps' file reads key by key, and what it lacks keeps its default", () => {
  const settings = decodeSettings(
    JSON.stringify({
      meters: { vram: false, drive_activity: false, unknown: true },
      notifications: { job_finished: true, killed_outside_flyout: true },
      resources_expanded: true,
      first_run_shown: true,
      window: { x: 3 },
    }),
  );

  assert.equal(settings.meters.vram, false);
  assert.equal(settings.meters.drive_activity, false);
  assert.equal(settings.meters.cpu, true);
  assert.equal(settings.notifications.job_finished, true);
  assert.equal(settings.notifications.killed_outside_flyout, true);
  assert.equal(settings.notifications.running_long, true);
  assert.equal(settings.resources_expanded, true);
  assert.equal(settings.first_run_shown, true);
  assert.equal(settings.first_run_done, false);
  assert.equal("window" in settings, false);
  assert.equal("unknown" in settings.meters, false);
});

test("a key of the wrong type keeps its default", () => {
  const settings = decodeSettings('{"resources_expanded":"yes","meters":{"cpu":0}}');

  assert.equal(settings.resources_expanded, false);
  assert.equal(settings.meters.cpu, true);
});

test("saving and reading gives the same settings, in the shared format", () => {
  const settings = defaultSettings();
  settings.meters.ram = false;
  settings.first_run_done = true;

  const text = encodeSettings(settings);

  assert.deepEqual(decodeSettings(text), settings);
  assert.ok(text.endsWith("\n"));
  assert.ok(text.includes('"killed_outside_flyout": false'));
  assert.ok(text.includes('"resources_expanded": false'));
});

test("Drive activity does not count as a meter", () => {
  const { meters } = defaultSettings();
  meters.drive_activity = false;
  assert.equal(meterCount(meters), "5 of 5");

  meters.gpu = false;
  assert.equal(meterCount(meters), "4 of 5");
});
