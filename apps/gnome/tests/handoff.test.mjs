import assert from "node:assert/strict";
import { test } from "node:test";

import { matchingApp, sandboxedAppId } from "../allot@mehran.tech/lib/app-match.js";
import { panelSnapshot } from "../allot@mehran.tech/lib/snapshot.js";
import { SCENARIO_NAMES, leaseId, scenario, stops } from "./handoff.mjs";

/** @import { JobCard } from "../allot@mehran.tech/lib/job-card.js" */

/**
 * The snapshot of a scenario as the fixture server serves it, before any press.
 *
 * @param {string} name
 */
function scenarioSnapshot(name) {
  const { status, state } = scenario(name);
  return panelSnapshot(status, state, { stops: stops(name) });
}

/**
 * @param {string} name
 * @param {string} command
 * @returns {JobCard}
 */
function scenarioCard(name, command) {
  const rows = scenarioSnapshot(name).groups.flatMap((group) => group.jobs);
  const row = rows.find((each) => each.command === command);
  assert.ok(row, `${name} has no ${command}`);
  return row.card;
}

test("every scenario's summary matches the handoff", () => {
  const cases = [
    ["empty", "Idle", "idle"],
    ["one", "1 running", "busy"],
    ["several", "2 running · 1 waiting", "busy"],
    ["kill-confirm", "2 running · 1 waiting", "busy"],
    ["killed", "1 running · 1 waiting", "busy"],
    ["bench-drain", "1 running · benchmark waiting", "busy"],
    ["bench-quiet", "Benchmark waiting for quiet", "idle"],
    ["bench-run", "Benchmark running alone", "busy"],
    ["bench-done", "1 waiting", "idle"],
    ["bench-noisy", "Benchmark waiting for quiet", "idle"],
    ["bench-forced", "Benchmark running alone", "busy"],
    ["down", "Service stopped", "error"],
    ["max", "4 running · 2 waiting", "busy"],
    ["icons", "4 running · 2 waiting", "busy"],
    ["c-active", "1 running", "busy"],
    ["c-over", "1 running", "busy"],
    ["c-queued", "2 waiting", "idle"],
    ["c-bench", "Benchmark waiting for quiet", "idle"],
    ["c-confirm", "1 running", "busy"],
    ["c-killed", "Idle", "idle"],
    ["c-removed", "Idle", "idle"],
    ["c-refused", "1 running", "busy"],
    ["first-run", "Idle", "idle"],
    ["res-shared", "1 running", "busy"],
  ];
  assert.deepEqual(
    cases.map(([name]) => name),
    SCENARIO_NAMES,
  );
  for (const [name, summary, glyph] of cases) {
    assert.deepEqual(scenarioSnapshot(String(name)).header, { summary, glyph }, name);
  }
});

test("active cards show the process, reservation and time", () => {
  const cases = [
    ["cargo build --release", "cargo 18244 · 8 cores · 4 GB", "3:06 / ~5:00", false],
    [
      "python train.py --epochs 20 --data ./shards",
      "python3.13 22108 · 2 cores · 4 GB · GPU 80% · 4 GB VRAM",
      "1:40 over",
      true,
    ],
    [
      "pnpm --filter web vite build --mode production",
      "node 9120 · 4 cores · 2 GB · GPU 10% · 1 GB VRAM",
      "0:52 / ~1:30",
      false,
    ],
    ["go vet ./...", "go 14772 · 1 core · 1 GB", "0:20 / ~1:00", false],
  ];
  for (const [command, detail, time, over] of cases) {
    const card = scenarioCard("max", String(command));

    assert.equal(card.detail, detail);
    assert.equal(card.time, time);
    assert.equal(card.timeIsWarning, over);
    assert.equal(card.barTone, over ? "over" : "running");
    assert.equal(card.action, "kill");
  }
});

test("active cards have initials, a chip and a kill name", () => {
  const cases = [
    ["cargo build --release", "CA", "CPU heavy", "Kill cargo"],
    ["python train.py --epochs 20 --data ./shards", "PY", "GPU heavy", "Kill python3.13"],
    ["pnpm --filter web vite build --mode production", "NO", "Mixed", "Kill node"],
    ["go vet ./...", "GO", "Light", "Kill go"],
  ];
  for (const [command, tile, chip, name] of cases) {
    const card = scenarioCard("max", command);

    assert.equal(card.tile, tile);
    assert.equal(card.chip, chip);
    assert.equal(card.actionName, name);
    assert.equal(card.isQueued || card.isBenchmark, false);
  }
});

test("a queued card with an estimate shows waited against the total", () => {
  const card = scenarioCard("several", "npm run test:e2e");

  assert.equal(card.tile, "#1");
  assert.ok(card.isQueued);
  assert.equal(card.detail, "2 cores · 4 GB");
  assert.equal(card.time, "0:42 / ~2 min");
  assert.ok(card.timeIsWarning);
  assert.equal(card.barTone, "waiting");
  assert.ok(Math.abs(card.barFraction - 0.35) < 0.001);
  assert.equal(card.action, "remove");
  assert.equal(card.actionName, "Remove npm run test:e2e from queue");
});

test("a queued card without an estimate depends on the queue", () => {
  const card = scenarioCard("c-queued", "cargo test --workspace");

  assert.equal(card.tile, "#2");
  assert.equal(card.detail, "8 cores · 4 GB");
  assert.equal(card.time, "0:18 · depends on queue");
  assert.equal(card.barTone, "waiting-dashed");
  assert.equal(card.chip, "Custom");
  assert.equal(card.chipTooltip, "Custom reservation");
});

test("a queued job behind a benchmark waits after it", () => {
  for (const name of ["bench-drain", "bench-quiet", "bench-run"]) {
    const card = scenarioCard(name, "npm run test:e2e");

    assert.equal(card.time, "1:12 · after benchmark", name);
    assert.equal(card.barTone, "waiting-dashed");
  }
});

test("a queued card of a session that also runs says so", () => {
  const rows = scenarioSnapshot("max").groups.flatMap((group) => group.jobs);
  /** @param {string} command */
  const usage = (command) => rows.find((row) => row.command === command)?.session.usage;

  assert.equal(usage("cargo build --release"), "7.8 cores · 2.9 GB");
  assert.equal(usage("cargo test --workspace"), "also running");
  assert.equal(usage("npm run test:e2e"), "0.4 cores · 1.1 GB");
});

test("a draining benchmark has the status line and stepper", () => {
  const card = scenarioCard("bench-drain", "cargo bench --bench parser");

  assert.equal(card.detail, "Draining · waiting for running jobs to finish");
  assert.equal(card.time, "");
  assert.equal(card.icon, "timer");
  assert.ok(card.isBenchmark);
  assert.equal(card.chip, "Benchmark");
  assert.deepEqual(card.steps, { current: "drain", quietTicks: 0, runFraction: 0 });
  assert.equal(card.barTone, "none");
  assert.equal(card.action, "remove");
  assert.equal(card.actionName, "Remove cargo bench --bench parser from queue");
});

test("a quiet benchmark counts ticks, a noisy one says when it starts anyway", () => {
  const quiet = scenarioCard("bench-quiet", "cargo bench --bench parser");
  const noisy = scenarioCard("bench-noisy", "cargo bench --bench parser");

  assert.equal(quiet.detail, "Waiting for quiet · 6 / 10 s · background 3% CPU");
  assert.deepEqual(quiet.steps, { current: "quiet", quietTicks: 6, runFraction: 0 });
  assert.equal(
    noisy.detail,
    "Not quiet · background 14% CPU (limit 5%) · starting anyway in 0:42",
  );
  assert.equal(noisy.steps?.quietTicks, 10);
});

test("a running benchmark runs alone, a forced one was not quiet at start", () => {
  const row = scenarioSnapshot("bench-run").groups.find((group) => group.kind === "benchmark")
    ?.jobs[0];
  assert.ok(row);

  assert.equal(row.card.detail, "Running alone · 0:48 / ~2:00");
  assert.equal(row.card.tile, "CA");
  assert.equal(row.card.action, "kill");
  assert.equal(row.card.steps?.current, "run");
  assert.ok(Math.abs((row.card.steps?.runFraction ?? 0) - 0.4) < 0.001);
  assert.equal(row.session.usage, "15.6 cores · 3.1 GB");
  assert.equal(
    scenarioCard("bench-forced", "cargo bench --bench parser").detail,
    "Running alone · not quiet at start · 0:48 / ~2:00",
  );
});

test("a finished benchmark holds with its noise", () => {
  const snapshot = scenarioSnapshot("bench-done");
  const row = snapshot.groups.find((group) => group.kind === "benchmark")?.jobs[0];
  assert.ok(row);

  assert.equal(row.id, leaseId("bench"));
  assert.ok(row.isEnding);
  assert.equal(row.session.project, "parser-perf");
  assert.equal(row.card.detail, "Finished in 2:04 · background noise avg 1.8%, peak 6.2% CPU");
  assert.equal(row.card.icon, "check");
  assert.deepEqual(row.card.steps, { current: "done", quietTicks: 10, runFraction: 1 });
  assert.equal(row.card.action, "none");
  const waiting = snapshot.groups.find((group) => group.kind === "waiting");
  assert.equal(waiting?.jobs[0]?.card.time, "starts next · ~1 min");
});

test("the empty scenario lists recent runs", () => {
  const snapshot = scenarioSnapshot("empty");

  assert.deepEqual(snapshot.groups, []);
  assert.deepEqual(
    snapshot.recent.map((line) => [line.command, line.shortId, line.meta]),
    [
      ["cargo build --release", "3f2a9c1e", "4:52 · 4 min ago"],
      ["npm run test:e2e", "77a0e5d2", "2:10 · 11 min ago"],
    ],
  );
});

test("down shows the last known job frozen", () => {
  const card = scenarioCard("down", "cargo build --release");

  assert.equal(card.detail, "cargo 18244 · was at 3:06 / ~5:00");
  assert.equal(card.time, "");
  assert.equal(card.barTone, "frozen");
  assert.equal(card.action, "none");
});

test("a confirm scenario asks Kill?", () => {
  for (const name of ["kill-confirm", "c-confirm"]) {
    const card = scenarioCard(name, "cargo build --release");

    assert.equal(card.strong, "");
    assert.equal(card.detail, "Stops cargo and its child processes");
    assert.ok(card.detailIsDanger);
    assert.equal(card.time, "3s");
    assert.equal(card.action, "confirm");
    assert.equal(card.actionName, "Confirm kill cargo");
  }
});

test("a killed scenario holds Stopped", () => {
  for (const name of ["killed", "c-killed"]) {
    const card = scenarioCard(name, "cargo build --release");

    assert.equal(card.strong, "Stopped");
    assert.equal(card.detail, " · Claude was notified");
    assert.equal(card.time, "at 3:06");
    assert.equal(card.barTone, "frozen");
    assert.ok(card.isEnded && card.isStruck);
    assert.equal(card.action, "none");
    assert.equal(card.announcement, "Stopped cargo · Claude was notified");
  }
});

test("the removed scenario holds Removed", () => {
  const card = scenarioCard("c-removed", "npm run test:e2e");

  assert.equal(card.tile, "#1");
  assert.equal(card.strong, "Removed");
  assert.equal(card.detail, " · Claude was notified");
  assert.equal(card.time, "");
  assert.equal(card.barTone, "none");
  assert.equal(card.announcement, "Removed npm run test:e2e · Claude was notified");
});

test("the icons scenario points running jobs at installed apps", () => {
  const apps = [
    { id: "org.gnome.Settings.desktop", exec: "gnome-control-center", tryExec: "" },
    { id: "byobu.desktop", exec: "env TERM=xterm-256color byobu", tryExec: "" },
    { id: "vim.desktop", exec: "vim %F", tryExec: "vim" },
  ];
  const rows = scenarioSnapshot("icons").groups.flatMap((group) => group.jobs);
  const matched = rows
    .filter((row) => !row.isQueued)
    .map((row) => {
      const exe = row.card.exePath ?? "";
      return sandboxedAppId(exe) ?? matchingApp(exe, apps);
    });

  assert.deepEqual(matched, [
    "org.gnome.Settings.desktop",
    "byobu.desktop",
    "vim.desktop",
    null,
  ]);
});

test("groups come in benchmark, running, waiting order", () => {
  const snapshot = scenarioSnapshot("bench-drain");

  assert.deepEqual(
    snapshot.groups.map((group) => [group.kind, group.heading]),
    [
      ["benchmark", "Benchmark"],
      ["running", "Running · 1"],
      ["waiting", "Waiting · 1"],
    ],
  );
  assert.equal(snapshot.benchmark, "draining");
});
