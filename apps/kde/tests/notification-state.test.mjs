import assert from "node:assert/strict";
import { test } from "node:test";

import { isOffInSettings, parseInhibited } from "../src/notification-state.js";

test("Do Not Disturb reads from gdbus's answer", () => {
  assert.equal(parseInhibited("(<true>,)\n"), true);
  assert.equal(parseInhibited("(<false>,)\n"), false);
});

test("without settings files, notifications are on", () => {
  assert.equal(isOffInSettings(null, null), false);
  assert.equal(isOffInSettings("", "[Applications][other]\nShowPopups=false\n"), false);
});

test("the app's switch in plasmanotifyrc turns them off", () => {
  const off = "[Services][allot]\nSeen=true\nShowPopups=false\n\n[Other]\nShowPopups=true\n";

  assert.equal(isOffInSettings(null, off), true);
  assert.equal(isOffInSettings(null, "[Services][allot]\nShowPopups=true\n"), false);
});

test("an event without a popup action turns them off", () => {
  assert.equal(isOffInSettings("[Event/notification]\nAction=Sound\n", null), true);
  assert.equal(isOffInSettings("[Event/notification]\nAction=\n", null), true);
  assert.equal(isOffInSettings("[Event/notification]\nAction=Sound|Popup\n", null), false);
  assert.equal(isOffInSettings("[Event/other]\nAction=\n", null), false);
});
