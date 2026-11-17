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

test("duration reads minutes, then hours", () => {
  const cases = [
    [0, "0:00"],
    [-5, "0:00"],
    [186, "3:06"],
    [3599, "59:59"],
    [3725, "1:02:05"],
  ];
  for (const [seconds, expected] of cases) {
    assert.equal(format.duration(Number(seconds)), expected);
  }
});

test("minutes round halves away from zero and never fall below one", () => {
  const cases = [
    [0, "1 min"],
    [29, "1 min"],
    [90, "2 min"],
    [130, "2 min"],
    [3600, "1 h"],
    [4800, "1 h 20 min"],
  ];
  for (const [seconds, expected] of cases) {
    assert.equal(format.minutes(Number(seconds)), expected);
  }
});

test("ago names the largest unit", () => {
  const cases = [
    [59, "just now"],
    [60, "1 min ago"],
    [3599, "59 min ago"],
    [7200, "2 h ago"],
    [259_200, "3 d ago"],
  ];
  for (const [seconds, expected] of cases) {
    assert.equal(format.ago(Number(seconds)), expected);
  }
});

test("initials are the first two letters or digits of the program", () => {
  const cases = [
    ["cargo", "CA"],
    ["/usr/bin/python3.13", "PY"],
    ["cargo.exe", "CA"],
    ['"node"', "NO"],
    ["C:\\tools\\go.exe", "GO"],
    ["x", "X"],
    ["--", ""],
    ["", ""],
  ];
  for (const [program, expected] of cases) {
    assert.equal(format.initials(program), expected, program);
  }
});

test("cores drop a zero decimal", () => {
  const cases = [
    [1, "1 core"],
    [8, "8 cores"],
    [2.5, "2.5 cores"],
    [0.25, "0.3 cores"],
  ];
  for (const [cores, expected] of cases) {
    assert.equal(format.cores(Number(cores)), expected);
  }
});

test("a reservation shows only what is reserved", () => {
  /** @param {number} cores @param {number} ram @param {number} gpu @param {number} vram */
  const profile = (cores, ram, gpu, vram) => ({
    cores,
    ram_mb: ram,
    gpu_pct: gpu,
    vram_mb: vram,
    est_secs: 1,
  });

  assert.equal(format.reservation(profile(8, 4096, 0, 0)), "8 cores · 4 GB");
  assert.equal(
    format.reservation(profile(2, 4096, 80, 4096)),
    "2 cores · 4 GB · GPU 80% · 4 GB VRAM",
  );
  assert.equal(format.reservation(profile(1, 1536, 0, 0)), "1 core · 1.5 GB");
  assert.equal(format.reservation(profile(4, 0, 0, 0)), "4 cores");
});

test("granted names the asked cores only when fewer", () => {
  const cases = [
    [5.6, 8, "5.6 of 8 cores"],
    [0.5, 1, "0.5 of 1 cores"],
    [8, 8, "8 cores"],
    [7.96, 8, "8 cores"],
    [1, null, "1 core"],
  ];
  for (const [granted, asked, expected] of cases) {
    assert.equal(format.granted(Number(granted), asked === null ? null : Number(asked)), expected);
  }
});

test("stats show median and max, with GPU and VRAM only when used", () => {
  const median = { cores: 0.85, ram_mb: 1229, gpu_pct: 0, vram_mb: 0, est_secs: 190 };
  const max = { ...median, cores: 3.4, ram_mb: 2048, est_secs: 292 };

  assert.equal(
    format.stats({ runs: 12, median, p90: max, max }),
    "median 0.85 cores · 1.2 GB · 3:10   max 3.4 cores · 2 GB · 4:52",
  );
  assert.equal(
    format.stats({
      runs: 12,
      median: { ...median, cores: 1 },
      p90: max,
      max: { ...max, gpu_pct: 40, vram_mb: 1024 },
    }),
    "median 1 core · 1.2 GB · GPU 0% · 0 GB VRAM · 3:10   " +
      "max 3.4 cores · 2 GB · GPU 40% · 1 GB VRAM · 4:52",
  );
});
