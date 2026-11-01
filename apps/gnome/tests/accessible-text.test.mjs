import assert from "node:assert/strict";
import { test } from "node:test";

import {
  Announcer,
  cardDescription,
  detailText,
  driveDescription,
  driveName,
  meterColumns,
  meterName,
} from "../allot@mehran.tech/lib/accessible-text.js";

/** @import { JobCard } from "../allot@mehran.tech/lib/job-card.js" */
/** @import { JobRow, PanelSnapshot } from "../allot@mehran.tech/lib/snapshot.js" */

/**
 * @param {Partial<JobCard>} card
 * @param {number} [id]
 * @returns {JobRow}
 */
function row(card, id = 1) {
  return /** @type {JobRow} */ ({
    id,
    command: "cargo build",
    session: { shortId: "a1b2c3d4", project: "allot", projectPath: "", usage: "1.5 cores" },
    isQueued: false,
    isEnding: false,
    card: /** @type {JobCard} */ ({
      chip: "CPU heavy",
      strong: "",
      detail: "cargo 42 · 4 cores",
      detailTooltip: "",
      time: "1:05",
      announcement: "",
      ...card,
    }),
  });
}

/**
 * @param {string} summary
 * @param {JobRow[]} jobs
 * @returns {PanelSnapshot}
 */
function snapshot(summary, jobs = []) {
  return /** @type {PanelSnapshot} */ ({
    header: { glyph: "busy", summary },
    groups: [{ kind: "running", heading: "Running", jobs }],
  });
}

test("a card reads as its session, command, state and time", () => {
  assert.equal(
    cardDescription(row({})),
    "a1b2c3d4, allot, 1.5 cores, cargo build, CPU heavy, cargo 42 · 4 cores, 1:05",
  );
});

test("empty parts leave no stray commas", () => {
  const job = row({ time: "" });
  job.session = { ...job.session, project: "", usage: "" };

  assert.equal(cardDescription(job), "a1b2c3d4, cargo build, CPU heavy, cargo 42 · 4 cores");
});

test("the detail keeps its bold start and prefers the full text", () => {
  assert.equal(detailText(row({ strong: "Stopped", detail: " · Claude was notified" })),
    "Stopped · Claude was notified");
  assert.equal(detailText(row({ detail: "cut…", detailTooltip: "not cut" })), "not cut");
});

test("meter and drive rows read as their label and value", () => {
  const meter = { label: "CPU", value: "64%" };
  const drive = { id: "/home", label: "home", value: "612 / 953 GB", activity: "R 42 · W 8 MB/s" };

  assert.equal(meterName(/** @type {any} */ (meter)), "CPU 64%");
  assert.equal(driveName(/** @type {any} */ (drive)), "home 612 / 953 GB");
  assert.equal(driveDescription(/** @type {any} */ (drive)), "/home, R 42 · W 8 MB/s");
  assert.equal(driveDescription(/** @type {any} */ ({ ...drive, activity: "" })), "/home");
});

test("meter columns grow with the text and never squeeze the bar away", () => {
  assert.deepEqual(meterColumns(1), { label: 58, value: 108 });
  assert.deepEqual(meterColumns(1.25), { label: 73, value: 135 });
  assert.deepEqual(meterColumns(2), { label: 116, value: 146 });
  assert.deepEqual(meterColumns(3), { label: 174, value: 88 });
});

test("a text size below the default keeps the handoff's columns", () => {
  assert.deepEqual(meterColumns(0.8), meterColumns(1));
});

test("what a popup shows on opening is not announced", () => {
  const announcer = new Announcer();

  assert.deepEqual(
    announcer.observe(snapshot("1 running", [row({ announcement: "Stopped cargo" })])),
    [],
  );
});

test("a stop outcome is announced once", () => {
  const announcer = new Announcer();
  announcer.observe(snapshot("1 running", [row({})]));

  const stopped = snapshot("1 running", [row({ announcement: "Stopped cargo" })]);
  assert.deepEqual(announcer.observe(stopped), ["Stopped cargo"]);
  assert.deepEqual(announcer.observe(stopped), []);
});

test("a changed outcome is announced again", () => {
  const announcer = new Announcer();
  announcer.observe(snapshot("1 running", [row({})]));

  announcer.observe(snapshot("1 running", [row({ announcement: "cargo not stopped: busy" })]));
  assert.deepEqual(announcer.observe(snapshot("1 running", [row({ announcement: "Stopped" })])), [
    "Stopped",
  ]);
});

test("the summary is announced when it changes, after the card that caused it", () => {
  const announcer = new Announcer();
  announcer.observe(snapshot("1 running", [row({})]));

  assert.deepEqual(announcer.observe(snapshot("1 running", [row({})])), []);
  assert.deepEqual(
    announcer.observe(snapshot("Nothing running", [row({ announcement: "Stopped cargo" })])),
    ["Stopped cargo", "Nothing running"],
  );
});

test("opening again starts from what is shown then", () => {
  const announcer = new Announcer();
  announcer.observe(snapshot("1 running", [row({})]));
  announcer.reset();

  assert.deepEqual(
    announcer.observe(snapshot("2 running", [row({ announcement: "Stopped cargo" })])),
    [],
  );
});
