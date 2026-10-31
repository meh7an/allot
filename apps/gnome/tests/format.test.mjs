import assert from "node:assert/strict";
import { test } from "node:test";

import * as format from "../allot@mehran.tech/lib/format.js";

test("shortId takes eight characters", () => {
  assert.equal(format.shortId("3f2a9c1e-77b0-4f6e-a2d8-5c9e1b4f7a02"), "3f2a9c1e");
  assert.equal(format.shortId("short"), "short");
  assert.equal(format.shortId(""), "");
});

test("project is the last folder of either path style", () => {
  const cases = [
    ["/home/user/work/allot", "allot"],
    ["/home/user/work/allot/", "allot"],
    ["E:\\work\\app", "app"],
    ["E:\\work\\app\\", "app"],
    ["/", ""],
    ["", ""],
  ];
  for (const [cwd, expected] of cases) {
    assert.equal(format.project(cwd), expected, cwd);
  }
  assert.equal(format.project(null), "");
});

test("usage rounds halves away from zero", () => {
  const cases = [
    [7.75, 2970, "7.8 cores · 2.9 GB"],
    [2.25, 1024, "2.3 cores · 1.0 GB"],
    [0, 0, "0.0 cores · 0.0 GB"],
    [16, 32768, "16.0 cores · 32.0 GB"],
  ];
  for (const [cores, ramMb, expected] of cases) {
    const usage = { cores: Number(cores), ram_mb: Number(ramMb), gpu_pct: 0, vram_mb: 0 };
    assert.equal(format.usage(usage), expected);
  }
});
