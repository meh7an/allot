import assert from "node:assert/strict";
import { test } from "node:test";

import { GLIDE_MIN_PX, barMove } from "../allot@mehran.tech/lib/bar-move.js";

test("a move too small to see waits", () => {
  assert.equal(barMove(0.5, 0.5, 300), "stay");
  assert.equal(barMove(0.5, 0.501, 300), "stay");
});

test("a slow job's bar steps without gliding", () => {
  assert.equal(barMove(0.5, 0.504, 300), "jump");
  assert.equal(barMove(0.5, 0.5 + (GLIDE_MIN_PX - 0.1) / 300, 300), "jump");
});

test("a move of a few pixels glides, either way", () => {
  assert.equal(barMove(0.5, 0.5 + GLIDE_MIN_PX / 300, 300), "glide");
  assert.equal(barMove(0.9, 0.1, 300), "glide");
});

test("a bar not laid out yet takes any new share at once", () => {
  assert.equal(barMove(0, 0.5, 0), "jump");
  assert.equal(barMove(0.5, 0.5, 0), "stay");
});
