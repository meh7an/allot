import assert from "node:assert/strict";
import { test } from "node:test";

import { driveLabel, resourcesView } from "../allot@mehran.tech/lib/resources-view.js";
import { defaultSettings } from "../allot@mehran.tech/lib/settings.js";
import { scenario } from "./handoff.mjs";
import { fixtureState } from "./support.mjs";

/** @import { MeterToggles } from "../allot@mehran.tech/lib/settings.js" */

/**
 * @param {string} name
 * @param {Partial<MeterToggles>} [toggles]
 * @param {boolean} [expanded]
 * @returns {import("../allot@mehran.tech/lib/resources-view.js").ResourcesView}
 */
function view(name, toggles = {}, expanded = true) {
  const { status, state } = scenario(name);
  const meters = { ...defaultSettings().meters, ...toggles };
  return resourcesView(status, state, meters, expanded);
}

/**
 * @param {import("../allot@mehran.tech/lib/resources-view.js").ResourcesView} card
 * @param {string} id
 * @returns {import("../allot@mehran.tech/lib/resources-view.js").MeterRow}
 */
function row(card, id) {
  const found = card.meters.find((meter) => meter.id === id);
  assert.ok(found, `no row ${id}`);
  return found;
}

test("there is no card without live data", () => {
  const { state } = scenario("one");

  assert.equal(resourcesView("stopped", state, defaultSettings().meters, true).isVisible, false);
  assert.equal(resourcesView("running", null, defaultSettings().meters, true).isVisible, false);
});

test("collapsed shows the summary line, expanded none", () => {
  assert.equal(view("several", {}, false).summary, "CPU 64% · RAM 22.4 / 32 GB · GPU 78%");
  assert.equal(view("several", {}, false).isExpanded, false);
  assert.equal(view("several").summary, "");
  assert.equal(view("several", { gpu: false, ram: false }, false).summary, "CPU 64%");
});

test("rows follow the handoff: CPU, RAM, GPU, VRAM, with values and details", () => {
  const card = view("several");

  assert.deepEqual(
    card.meters.map((meter) => meter.label),
    ["CPU", "RAM", "GPU", "VRAM"],
  );
  const cpu = row(card, "cpu");
  assert.equal(cpu.value, "64%");
  assert.equal(cpu.detail, "10 of 16 cores reserved");
  assert.equal(cpu.reserved, 10 / 16);
  assert.equal(cpu.headroom, 0.85);
  assert.equal(cpu.spark.length, 24);
  const ram = row(card, "ram");
  assert.equal(ram.value, "22.4 / 32 GB");
  assert.equal(ram.detail, "8 GB reserved");
  assert.equal(ram.reserved, 0.25);
  assert.equal(ram.shared, null);
  assert.equal(row(card, "gpu:0").detail, "80% reserved");
  assert.equal(row(card, "vram:0").value, "7.2 / 11 GB");
  assert.equal(card.showsLegend, true);
  assert.equal(card.showsSharedLegend, false);
  assert.equal(card.sharesMemory, false);
});

test("nothing reserved reads No reservations and has no tick or legend", () => {
  const card = view("empty");

  assert.equal(row(card, "cpu").reserved, null);
  assert.equal(row(card, "cpu").detail, "No reservations");
  assert.equal(card.showsLegend, false);
});

test("a bar without a reservation next to one that has one reads Not reserved", () => {
  assert.equal(row(view("one"), "gpu:0").detail, "Not reserved");
});

test("a benchmark running alone reserves the whole machine", () => {
  const card = view("bench-run");

  assert.equal(row(card, "cpu").reserved, 1);
  assert.equal(row(card, "cpu").detail, "Whole machine reserved for the benchmark");
  assert.equal(row(card, "ram").detail, "Whole machine reserved");
  assert.equal(row(card, "gpu:0").reserved, 1);
  assert.equal(row(card, "vram:0").detail, "Whole machine reserved");
});

test("several adapters are numbered and named in the detail", () => {
  const card = view("max");

  assert.deepEqual(
    card.meters.map((meter) => meter.label),
    ["CPU", "RAM", "GPU 0", "VRAM 0", "GPU 1", "VRAM 1"],
  );
  assert.equal(row(card, "gpu:0").detail, "NVIDIA RTX 2080 Ti · 90% reserved");
  assert.equal(row(card, "gpu:1").detail, "Intel UHD Graphics · Not reserved");
});

test("a GPU sharing RAM has no VRAM row and tints the RAM bar", () => {
  const card = view("res-shared");

  assert.deepEqual(
    card.meters.map((meter) => meter.id),
    ["cpu", "ram", "gpu:0"],
  );
  const ram = row(card, "ram");
  assert.equal(ram.detail, "4.5 GB reserved · GPU uses 1.2 GB of it");
  assert.equal(ram.reserved, 4.5 / 32);
  assert.ok(Math.abs((ram.shared ?? 0) - 1.2 / 32) < 1e-3);
  assert.equal(card.sharesMemory, true);
  assert.equal(card.showsSharedLegend, true);
});

test("with VRAM off a shared GPU's tint goes too", () => {
  const ram = row(view("res-shared", { vram: false }), "ram");

  assert.equal(ram.shared, null);
  assert.equal(ram.detail, "4.5 GB reserved");
});

test("VRAM without a reported total has an empty bar and the used figure", () => {
  const { state } = scenario("one");
  const [gpu] = state.meters.gpus;
  assert.ok(gpu);
  gpu.vram_total_mb = null;

  const vram = row(resourcesView("running", state, defaultSettings().meters, true), "vram:0");

  assert.equal(vram.used, 0);
  assert.equal(vram.reserved, null);
  assert.equal(vram.value, "2.5 GB");
  assert.equal(vram.detail, "Not reserved · total not reported");
});

test("drives show the mount point's last part, space and rates", () => {
  const card = view("max");

  assert.deepEqual(
    card.drives.map((drive) => [drive.id, drive.label, drive.value, drive.activity]),
    [
      ["/", "/", "612 / 953 GB", "R 42 · W 8 MB/s"],
      ["/home", "home", "1.21 / 1.82 TB", "Idle"],
      ["/mnt/backup", "backup", "0.39 / 1.82 TB", "W 120 MB/s"],
    ],
  );
  assert.equal(card.showsSeparator, true);
});

test("rates have a decimal below 10 MB/s and vanish under 0.1", () => {
  const { state } = scenario("one");
  const [drive] = state.meters.drives;
  assert.ok(drive);
  drive.read_bytes_per_sec = 1.55 * 1024 * 1024;
  drive.write_bytes_per_sec = 50_000;

  const card = resourcesView("running", state, defaultSettings().meters, true);

  assert.equal(card.drives[0]?.activity, "R 1.6 MB/s");
});

test("Drive activity off leaves the rates empty", () => {
  const [drive] = view("max", { drive_activity: false }).drives;

  assert.equal(drive?.activity, "");
});

test("every meter off says how to turn them on", () => {
  const card = view("max", { cpu: false, ram: false, gpu: false, vram: false, drives: false });

  assert.equal(card.allOff, true);
  assert.deepEqual(card.meters, []);
  assert.deepEqual(card.drives, []);
  assert.equal(card.showsSeparator, false);
});

test("a Linux fixture reads through", () => {
  const card = resourcesView("running", fixtureState("linux"), defaultSettings().meters, true);

  assert.equal(card.isVisible, true);
  assert.ok(card.meters.length >= 2);
});

test("a drive label is the last part of the mount point", () => {
  assert.equal(driveLabel("/"), "/");
  assert.equal(driveLabel("/run/media/user/STICK"), "STICK");
});
