import assert from "node:assert/strict";
import { test } from "node:test";

import {
  benchmarkColor,
  benchmarkInk,
  detailMarkup,
  detailWraps,
  isDark,
  scaledMs,
  stepState,
  withAlpha,
} from "../src/card-view.js";

/**
 * @param {Record<string, unknown>} fields
 * @returns {any}
 */
function card(fields) {
  return { strong: "", detail: "", icon: "none", ...fields };
}

test("a duration follows Plasma's animation speed and ends at Instant", () => {
  assert.equal(scaledMs(240, 200), 240);
  assert.equal(scaledMs(240, 100), 120);
  assert.equal(scaledMs(240, 400), 480);
  assert.equal(scaledMs(240, 0), 0);
});

test("a colour with an opacity reads as #aarrggbb", () => {
  assert.equal(withAlpha({ r: 1, g: 0, b: 0.5 }, 0.5), "#80ff0080");
  assert.equal(withAlpha({ r: 0, g: 0, b: 0 }, 0.04), "#0a000000");
});

test("the Benchmark purple and its ink follow the background's lightness", () => {
  const dark = isDark({ r: 0.1, g: 0.1, b: 0.12 });
  const light = isDark({ r: 0.95, g: 0.95, b: 0.95 });

  assert.equal(dark, true);
  assert.equal(light, false);
  assert.equal(benchmarkColor(dark), "#c9a8f0");
  assert.equal(benchmarkInk(dark), "#1a1a1a");
  assert.equal(benchmarkColor(light), "#6f3fb0");
  assert.equal(benchmarkInk(light), "#ffffff");
});

test("the detail line bolds its start and escapes what comes from outside", () => {
  assert.equal(detailMarkup(card({ detail: "cargo 1 · 8 cores" })), "cargo 1 · 8 cores");
  assert.equal(
    detailMarkup(card({ strong: "Stopped", detail: " · Claude was notified" })),
    "<b>Stopped</b> · Claude was notified",
  );
  assert.equal(detailMarkup(card({ detail: "a<b> & <c" })), "a&lt;b&gt; &amp; &lt;c");
});

test("only a line with an icon wraps", () => {
  assert.equal(detailWraps(card({})), false);
  assert.equal(detailWraps(card({ icon: "timer" })), true);
  assert.equal(detailWraps(card({ icon: "check" })), true);
});

test("each step is past, current or upcoming", () => {
  assert.equal(stepState("drain", "quiet"), "past");
  assert.equal(stepState("quiet", "quiet"), "current");
  assert.equal(stepState("run", "quiet"), "upcoming");
  assert.equal(stepState("run", "done"), "past");
});
