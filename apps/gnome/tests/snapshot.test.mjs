import assert from "node:assert/strict";
import { test } from "node:test";

import { panelSnapshot } from "../allot@mehran.tech/lib/snapshot.js";
import { active, buildState, fixtureState, queued, session } from "./support.mjs";

const ALICE = "a1b2c3d4-0000-4000-8000-000000000001";
const BOB = "b0b0b0b0-0000-4000-8000-000000000002";
const CAROL = "c0c0c0c0-0000-4000-8000-000000000003";

test("every fixture's header matches the Windows summary", () => {
  const cases = [
    ["first-run", "Idle", "idle"],
    ["idle", "Idle", "idle"],
    ["busy", "2 running · 2 waiting", "busy"],
    ["benchmark", "Benchmark waiting for quiet", "idle"],
    ["macos", "1 running · 1 waiting", "busy"],
    ["linux", "2 running · 1 waiting", "busy"],
  ];
  for (const [name, summary, glyph] of cases) {
    const snapshot = panelSnapshot("running", fixtureState(String(name)));

    assert.deepEqual(snapshot.header, { glyph, summary }, name);
    assert.equal(snapshot.isLastKnown, false);
  }
});

test("service down shows the error glyph and the last known jobs", () => {
  const cases = [
    ["stopped", "Service stopped"],
    ["starting", "Starting service…"],
    ["incompatible", "Service needs an update"],
  ];
  for (const [status, summary] of /** @type {const} */ (cases)) {
    const snapshot = panelSnapshot(status, fixtureState("busy"));

    assert.deepEqual(snapshot.header, { glyph: "error", summary });
    assert.equal(snapshot.isLastKnown, true);
    assert.deepEqual(
      snapshot.groups.map((group) => group.heading),
      ["Running · 2", "Waiting · 2"],
    );
    assert.equal(snapshot.showNothing, false);
  }
});

test("down without any state shows no jobs", () => {
  const snapshot = panelSnapshot("stopped", null);

  assert.deepEqual(snapshot.groups, []);
  assert.equal(snapshot.idle, null);
  assert.equal(snapshot.showNothing, false);
});

test("busy fixture rows carry their session strip", () => {
  const snapshot = panelSnapshot("running", fixtureState("busy"));
  const rows = snapshot.groups.flatMap((group) => group.jobs);

  assert.deepEqual(
    rows.map((row) => row.id),
    [42, 41, 43, 44],
  );
  assert.equal(rows[1]?.command, "cargo build --release");
  assert.deepEqual(rows[1]?.session, {
    shortId: "3f2a9c1e",
    project: "app",
    projectPath: "E:\\work\\app",
    usage: "7.8 cores · 2.9 GB",
  });
  // 43 waits for a session that also runs 42.
  assert.equal(rows[2]?.session.usage, "also running");
  assert.deepEqual(
    rows.map((row) => row.isQueued),
    [false, false, true, true],
  );
});

test("the idle line lists sessions without jobs", () => {
  const busy = panelSnapshot("running", fixtureState("busy"));
  const idle = panelSnapshot("running", fixtureState("idle"));

  assert.deepEqual(busy.idle, {
    label: "Idle · 1",
    ids: "7c19e2aa",
    usage: "0.3 cores · 0.5 GB",
  });
  assert.deepEqual(idle.idle, { label: "Idle · 2", ids: "3f2a9c1e  b81d44f0", usage: "" });
  assert.equal(idle.showNothing, true);
});

test("groups come in benchmark, running, waiting order", () => {
  const state = buildState({
    sessions: [session(ALICE), session(BOB), session(CAROL)],
    active: [active(1, ALICE)],
    queue: [queued(2, BOB, { benchmark: true, phase: "draining" }), queued(3, CAROL)],
  });

  const snapshot = panelSnapshot("running", state);

  assert.deepEqual(
    snapshot.groups.map((group) => [group.kind, group.heading]),
    [
      ["benchmark", "Benchmark"],
      ["running", "Running · 1"],
      ["waiting", "Waiting · 1"],
    ],
  );
  assert.equal(snapshot.header.summary, "1 running · benchmark waiting");
  assert.equal(snapshot.benchmark, "draining");
});

test("a queued benchmark's phase follows its verdict", () => {
  const cases = [
    ["draining", "draining", "Benchmark waiting"],
    ["measuring", "waiting-for-quiet", "Benchmark waiting for quiet"],
    ["noisy", "waiting-for-quiet", "Benchmark waiting for quiet"],
  ];
  for (const [verdict, phase, summary] of cases) {
    const state = buildState({ queue: [queued(1, ALICE, { benchmark: true, phase: verdict })] });

    const snapshot = panelSnapshot("running", state);

    assert.equal(snapshot.benchmark, phase);
    assert.deepEqual(snapshot.header, { glyph: "idle", summary });
  }
});

test("an active benchmark runs alone in the benchmark group", () => {
  const state = buildState({
    active: [active(1, ALICE, { label: "cargo bench", benchmark: true })],
    queue: [queued(2, BOB)],
  });

  const snapshot = panelSnapshot("running", state);

  assert.deepEqual(snapshot.header, { glyph: "busy", summary: "Benchmark running alone" });
  assert.deepEqual(
    snapshot.groups.map((group) => group.heading),
    ["Benchmark", "Waiting · 1"],
  );
  assert.deepEqual(
    snapshot.groups[0]?.jobs.map((job) => job.command),
    ["cargo bench"],
  );
});

test("a job of an unregistered session uses the lease cwd", () => {
  const state = buildState({ active: [active(1, ALICE, { cwd: "/home/user/work/allot/" })] });

  const strip = panelSnapshot("running", state).groups[0]?.jobs[0]?.session;

  assert.deepEqual(strip, {
    shortId: "a1b2c3d4",
    project: "allot",
    projectPath: "/home/user/work/allot/",
    usage: "",
  });
});

test("a session without cwd falls back to the lease cwd", () => {
  const state = buildState({
    sessions: [session(ALICE, { cwd: null })],
    active: [active(1, ALICE, { cwd: "/home/user/work/tools" })],
  });

  const strip = panelSnapshot("running", state).groups[0]?.jobs[0]?.session;

  assert.equal(strip?.project, "tools");
  assert.equal(strip?.usage, "1.5 cores · 2.0 GB");
});
