import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

import {
  ContractError,
  decodeCommandTest,
  decodeFitPreview,
  decodeKillResponse,
  decodeRemoveResponse,
  decodeSettingsRefusal,
  decodeSettingsSpec,
  decodeSettingsState,
  decodeStopRecord,
  decodeUiState,
} from "./contract.g.mjs";

const FIXTURE_DIR = new URL("../fixtures/", import.meta.url);

// Every fixture file names its root type by its prefix.
const DECODERS = [
  ["ui-state-", decodeUiState],
  ["kill-response-", decodeKillResponse],
  ["remove-response", decodeRemoveResponse],
  ["settings-spec", decodeSettingsSpec],
  ["stop-record", decodeStopRecord],
  ["settings-state", decodeSettingsState],
  ["settings-refusal-", decodeSettingsRefusal],
  ["fit-preview", decodeFitPreview],
  ["command-test", decodeCommandTest],
];

/**
 * @param {string} name
 * @returns {unknown}
 */
function readFixture(name) {
  return JSON.parse(readFileSync(new URL(`${name}.json`, FIXTURE_DIR), "utf8"));
}

/**
 * Fails unless every member of `expected`, at any depth, is in `actual` with the same value.
 * The decoder drops members it does not know, so a fixture member missing from the decoded
 * value is a field the generated types lack.
 *
 * @param {unknown} expected
 * @param {unknown} actual
 * @param {string} path
 */
function assertContains(expected, actual, path) {
  if (Array.isArray(expected)) {
    assert.ok(Array.isArray(actual), `${path}: not an array`);
    assert.equal(actual.length, expected.length, `${path}: length`);
    expected.forEach((item, index) => assertContains(item, actual[index], `${path}[${index}]`));
  } else if (typeof expected === "object" && expected !== null) {
    assert.ok(typeof actual === "object" && actual !== null, `${path}: not an object`);
    for (const [key, value] of Object.entries(expected)) {
      assert.ok(Object.hasOwn(actual, key), `${path}.${key}: dropped by the decoder`);
      assertContains(value, actual[key], `${path}.${key}`);
    }
  } else {
    assert.equal(actual, expected, path);
  }
}

const fixtures = readdirSync(FIXTURE_DIR)
  .filter((file) => file.endsWith(".json"))
  .map((file) => file.slice(0, -".json".length))
  .sort();

for (const name of fixtures) {
  test(`fixture ${name} decodes without dropping members`, () => {
    const [, decode] = DECODERS.find(([prefix]) => name.startsWith(prefix));
    const json = readFixture(name);

    assertContains(json, decode(json), name);
  });
}

test("Windows fixtures decode with the defaults allotd leaves out", () => {
  const state = decodeUiState(readFixture("ui-state-busy"));

  const [gpu] = state.meters.gpus;
  assert.equal(gpu.unified_memory, false);
  assert.equal(gpu.process_load, "exact");
  assert.equal(state.active[0].lease.isolation, null);
});

test("the Linux and macOS fixtures keep isolation and unified memory", () => {
  const linux = decodeUiState(readFixture("ui-state-linux"));
  const macos = decodeUiState(readFixture("ui-state-macos"));

  const isolation = linux.active[1].lease.isolation;
  assert.equal(isolation.kind, "cgroup");
  assert.equal(isolation.limits.cpu_cores, 8);
  assert.equal(isolation.limits.memory_max_mb, null);
  assert.equal(macos.meters.gpus[0].unified_memory, true);
  assert.equal(macos.meters.gpus[0].process_load, "estimated");
  assert.equal(macos.meters.gpus[0].vram_total_mb, null);
});

test("the benchmark fixture decodes its progress by phase", () => {
  const state = decodeUiState(readFixture("ui-state-benchmark"));

  assert.deepEqual(state.queue[0].verdict.benchmark, {
    phase: "measuring",
    measured_secs: 6,
    needed_secs: 10,
    cpu_avg_pct: 3,
    gpu_avg_pct: 0.5,
  });
});

test("the settings spec decodes every control kind and fills its optional members", () => {
  const spec = decodeSettingsSpec(readFixture("settings-spec"));
  const settings = new Map(
    spec.pages.flatMap((page) => page.groups.flatMap((group) => group.settings)).map((setting) => [
      setting.key,
      setting,
    ]),
  );

  const headroom = settings.get("admission.headroom_pct").control;
  assert.equal(headroom.kind, "number");
  assert.equal(headroom.quantity, "percent");
  assert.equal(headroom.default, 15);
  assert.equal(headroom.off, null);
  assert.equal(settings.get("leases.idle_release_secs").control.off.value, 0);
  assert.equal(settings.get("estimates.min_runs").requires.key, "estimates.lower");
  assert.equal(settings.get("admission.headroom_pct").requires, null);
  const classes = settings.get("classes").control;
  assert.equal(classes.kind, "size_table");
  assert.equal(classes.rows.length, 4);
  assert.equal(classes.new_row_name, null);
  assert.equal(settings.get("classifier.rules").control.default[0].class, "cpu_heavy");
  assert.equal(settings.get("classifier.rules").control.fields[0].control.pattern, null);
  assert.equal(settings.get("daemon.port").needs_restart, true);
  assert.deepEqual(settings.get("linux.scopes").platforms, ["linux"]);
});

test("the stop record decodes and rejects an unknown source", () => {
  const record = decodeStopRecord(readFixture("stop-record"));
  assert.deepEqual(record, { by: "panel", stopped_at_ms: 1_792_824_120_000 });

  const json = readFixture("stop-record");
  json.by = "tray";
  assert.throws(() => decodeStopRecord(json), ContractError);
});

test("an unknown setting control kind is rejected", () => {
  const json = readFixture("settings-spec");
  json.pages[0].groups[0].settings[0].control.kind = "slider";

  assert.throws(() => decodeSettingsSpec(json), ContractError);
});

test("an unknown enum value is rejected with its path", () => {
  const json = readFixture("ui-state-busy");
  json.active[0].lease.state = "paused";

  assert.throws(() => decodeUiState(json), {
    name: ContractError.name,
    message: 'UiState.active[0].lease.state: unknown value "paused"',
  });
});

test("an unknown benchmark phase is rejected", () => {
  const json = readFixture("ui-state-benchmark");
  json.queue[0].verdict.benchmark.phase = "warming";

  assert.throws(() => decodeUiState(json), ContractError);
});

test("a missing required field and a wrong type are rejected", () => {
  const missing = readFixture("remove-response");
  delete missing.lease.label;
  const negative = readFixture("remove-response");
  negative.lease.profile.ram_mb = -1;

  assert.throws(() => decodeRemoveResponse(missing), {
    message: "RemoveResponse.lease.label: missing",
  });
  assert.throws(() => decodeRemoveResponse(negative), ContractError);
});
