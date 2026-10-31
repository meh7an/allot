import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MARGIN,
  anchoredPosition,
  clampInto,
  maxHeight,
} from "../allot@mehran.tech/lib/placement.js";

const WORK_AREA = { x: 0, y: 32, width: 1920, height: 1048 };
const POPUP = { width: 360, height: 300 };

test("the popup is centred under its button", () => {
  const button = { x: 1000, y: 0, width: 40, height: 32 };

  assert.deepEqual(anchoredPosition(button, POPUP, WORK_AREA), { x: 840, y: 32 + MARGIN });
});

test("a button near the screen edge keeps the popup on screen", () => {
  const right = { x: 1880, y: 0, width: 40, height: 32 };
  const left = { x: 0, y: 0, width: 40, height: 32 };

  assert.equal(anchoredPosition(right, POPUP, WORK_AREA).x, 1920 - 360 - MARGIN);
  assert.equal(anchoredPosition(left, POPUP, WORK_AREA).x, MARGIN);
});

test("a second monitor's work area offsets the popup", () => {
  const area = { x: 1920, y: 0, width: 1280, height: 1024 };
  const button = { x: 3100, y: 0, width: 40, height: 32 };

  assert.deepEqual(anchoredPosition(button, POPUP, area), { x: 3200 - 360 - MARGIN, y: 38 });
});

test("dragging stays inside the area, top left first when too big", () => {
  assert.deepEqual(clampInto({ x: -50, y: 2000 }, POPUP, WORK_AREA), {
    x: MARGIN,
    y: 32 + 1048 - 300 - MARGIN,
  });
  assert.deepEqual(clampInto({ x: 10, y: 10 }, { width: 4000, height: 4000 }, WORK_AREA), {
    x: MARGIN,
    y: 32,
  });
});

test("height is capped at 640 or 70% of the work area", () => {
  assert.equal(maxHeight(WORK_AREA), 640);
  assert.equal(maxHeight({ x: 0, y: 32, width: 1280, height: 736 }), 515);
});
