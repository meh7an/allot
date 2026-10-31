import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { test } from "node:test";

import {
  INDICATOR_ICONS,
  accessibleName,
  iconFile,
  indicatorStatus,
} from "../allot@mehran.tech/lib/indicator-status.js";
import { panelSnapshot } from "../allot@mehran.tech/lib/snapshot.js";
import { EXTENSION_DIR, active, buildState, fixtureState, queued } from "./support.mjs";

const ALICE = "a1b2c3d4-0000-4000-8000-000000000001";
const BOB = "b0b0b0b0-0000-4000-8000-000000000002";

test("each fixture picks its icon", () => {
  const cases = [
    ["first-run", "idle"],
    ["idle", "idle"],
    ["busy", "queued"],
    ["benchmark", "benchmark"],
    ["linux", "queued"],
  ];
  for (const [name, icon] of cases) {
    const snapshot = panelSnapshot("running", fixtureState(String(name)));

    assert.equal(indicatorStatus(snapshot).icon, icon, name);
  }
});

test("running without waiting is busy", () => {
  const state = buildState({ active: [active(1, ALICE)] });

  const status = indicatorStatus(panelSnapshot("running", state));

  assert.deepEqual(status, { icon: "busy", summary: "1 running" });
  assert.equal(accessibleName(status), "Allot · 1 running");
});

test("a benchmark wins over waiting and running", () => {
  const state = buildState({
    active: [active(1, ALICE)],
    queue: [queued(2, BOB, { benchmark: true, phase: "draining" }), queued(3, ALICE)],
  });

  assert.equal(indicatorStatus(panelSnapshot("running", state)).icon, "benchmark");
});

test("down wins over the last known jobs", () => {
  for (const status of /** @type {const} */ (["stopped", "starting", "incompatible"])) {
    const snapshot = panelSnapshot(status, fixtureState("benchmark"));

    assert.equal(indicatorStatus(snapshot).icon, "down", status);
  }
});

test("every icon ships with the extension", () => {
  for (const icon of INDICATOR_ICONS) {
    assert.ok(existsSync(new URL(`icons/${iconFile(icon)}`, EXTENSION_DIR)), icon);
  }
});
