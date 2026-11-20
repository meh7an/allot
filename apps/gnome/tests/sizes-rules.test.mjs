import assert from "node:assert/strict";
import { test } from "node:test";

import { decodeCommandTest } from "../allot@mehran.tech/contract.g.mjs";
import { resetField } from "../allot@mehran.tech/lib/field-reset.js";
import {
  ANY_ARGUMENTS,
  BUILT_IN_RULES_TEXT,
  NO_COMMAND_TEXT,
  YOUR_RULES_TEXT,
  classLabels,
  matchedRules,
  ruleArguments,
  ruleDeletedText,
  ruleDialogTitle,
  ruleMenuName,
  ruleRowName,
  rulesStatus,
  testLines,
  useBuiltInConfirmation,
} from "../allot@mehran.tech/lib/rules-text.js";
import {
  NumberModel,
  RuleListModel,
  SizeTableModel,
  StringListModel,
} from "../allot@mehran.tech/lib/setting-values.js";
import {
  ADD_PRESET_TITLE,
  FITS_UNKNOWN,
  automaticMinCores,
  deletePresetConfirmation,
  deleteRowName,
  dialogFitsText,
  editRowName,
  fitsNowText,
  renameWarning,
  resetRowName,
  rowRemovedText,
  sizeCells,
  sizeDialogTitle,
  sizeSummary,
} from "../allot@mehran.tech/lib/sizes-text.js";
import {
  as,
  english,
  fire,
  fixtureState,
  loaded,
  setValue,
  withRender,
} from "./settings-support.mjs";
import { fixtureText } from "./support.mjs";

/**
 * @import { Change, HeavyRule, JobClass, SizeFit } from "../allot@mehran.tech/contract.g.mjs"
 */

// The Sizes and Commands pages' shared helpers (M6.1, ported from M4.2's and M5.2's tests):
// table cells and Fits now, the edit dialogs' titles, warnings and proposals, Undo after a row
// reset or a rule delete, and Test a command.

/**
 * @param {number} full
 * @param {number} shrunk
 * @param {boolean} [more]
 */
const counts = (full, shrunk, more = false) => ({ full, shrunk, more });

/**
 * @param {number} full
 * @param {number} shrunk
 * @param {boolean} [more]
 * @returns {SizeFit}
 */
const fit = (full, shrunk, more = false) => ({
  name: "cpu_heavy",
  min_cores: 4,
  now: counts(full, shrunk, more),
  idle: counts(0, 0),
});

/**
 * A change as the tests compare it: its kind, and the row's name for row changes.
 *
 * @param {Change} change
 */
function describe(change) {
  return change.kind === "set_row" || change.kind === "reset_row"
    ? `${change.kind} ${change.name}`
    : change.kind;
}

/** @param {JobClass} jobClass */
function labels(jobClass) {
  return { cpu_heavy: "CPU heavy", gpu_heavy: "GPU heavy", mixed: "Mixed", light: "Light" }[
    jobClass
  ];
}

const renderState = () => fixtureState(withRender);

test("size cells carry their units and the automatic minimum", () => {
  const big = {
    name: "big",
    profile: { cores: 8, ram_mb: 4096, gpu_pct: 50, vram_mb: 2048, est_secs: 300 },
    min_cores: null,
  };

  assert.deepEqual(sizeCells(big, english), {
    cores: "8",
    minCores: "Automatic (4)",
    memory: "4 GB",
    gpu: "50 %",
    videoMemory: "2 GB",
    runTime: "5 minutes",
  });

  const other = sizeCells(
    {
      name: "big",
      profile: { cores: 8, ram_mb: 1000, gpu_pct: 0, vram_mb: 0, est_secs: 45 },
      min_cores: 6,
    },
    english,
  );
  assert.equal(other.minCores, "6");
  assert.equal(other.memory, "1,000 MB");
  assert.equal(other.runTime, "45 seconds");
});

test("a row's summary reads as one line, with its fit when known", () => {
  const big = {
    name: "big",
    profile: { cores: 8, ram_mb: 4096, gpu_pct: 50, vram_mb: 2048, est_secs: 300 },
    min_cores: null,
  };

  assert.equal(
    sizeSummary(big, fit(1, 2), english),
    "8 cores, at least 4 (automatic) · 4 GB · GPU 50 % · 2 GB video memory · 5 minutes · " +
      "fits 1, 2 more shrunk",
  );
  assert.equal(
    sizeSummary({ ...big, min_cores: 6, profile: { ...big.profile, vram_mb: 0 } }, null, english),
    "8 cores, at least 6 · 4 GB · GPU 50 % · 5 minutes",
  );
});

test("the automatic minimum is half the cores, but at least one", () => {
  assert.equal(automaticMinCores(8), 4);
  assert.equal(automaticMinCores(1.5), 1);
  assert.equal(automaticMinCores(1), 1);
  assert.equal(automaticMinCores(0.5), 0.5);
  assert.equal(automaticMinCores(0), 0);
});

test("Fits now reads short in the table", () => {
  assert.equal(fitsNowText(null, english), FITS_UNKNOWN);
  assert.equal(fitsNowText(fit(0, 0), english), "None");
  assert.equal(fitsNowText(fit(3, 0), english), "3");
  assert.equal(fitsNowText(fit(0, 2), english), "2 shrunk");
  assert.equal(fitsNowText(fit(1, 2), english), "1, 2 more shrunk");
  assert.equal(fitsNowText(fit(99, 0, true), english), "at least 99");
  assert.equal(fitsNowText(fit(2, 99, true), english), "2, at least 99 more shrunk");
});

test("the dialog line names now and with every lease ended", () => {
  assert.equal(
    dialogFitsText(
      { name: "cpu_heavy", min_cores: 4, now: counts(1, 2), idle: counts(2, 0) },
      english,
    ),
    "With these values, now: 1 at full size, 2 more at 4 cores. With every lease ended: 2 at " +
      "full size.",
  );
  assert.ok(
    dialogFitsText(
      { name: "cpu_heavy", min_cores: 4, now: counts(0, 0), idle: counts(2, 0) },
      english,
    ).startsWith("With these values, now: none fit."),
  );
});

test("deleting a preset is confirmed in title case", () => {
  assert.deepEqual(deletePresetConfirmation("render"), {
    title: "Delete preset render?",
    body: "Sessions that ask for it by name get an error. You can add it again later.",
    button: "Delete Preset",
  });
});

test("dialog titles name the class or preset", async () => {
  const { session } = await loaded({ state: renderState() });
  const classes = as(session.setting("classes"), SizeTableModel);
  const presets = as(session.setting("presets"), SizeTableModel);

  const cpu = classes.edit("cpu_heavy");
  const render = presets.edit("render");
  assert.ok(cpu !== null && render !== null);
  assert.equal(sizeDialogTitle(classes, cpu), "Edit CPU heavy");
  assert.equal(sizeDialogTitle(presets, render), "Edit preset render");
  assert.equal(sizeDialogTitle(presets, presets.add()), ADD_PRESET_TITLE);
});

test("renaming a preset warns about sessions that ask for the old name", async () => {
  const { session } = await loaded({ state: renderState() });
  const presets = as(session.setting("presets"), SizeTableModel);
  const draft = presets.edit("render");
  const small = presets.edit("cpu_small");
  assert.ok(draft !== null && small !== null);

  assert.equal(renameWarning(draft), null);
  draft.setName("render_big");
  assert.equal(
    renameWarning(draft),
    "Sessions that ask for render get an error until they use render_big.",
  );
  assert.equal(renameWarning(small), null);
});

test("row actions name their row and say what happened", async () => {
  const { session } = await loaded({ state: renderState() });
  const classes = as(session.setting("classes"), SizeTableModel);
  const presets = as(session.setting("presets"), SizeTableModel);
  const cpu = classes.rows.find((view) => view.row.name === "cpu_heavy");
  const small = presets.rows.find((view) => view.row.name === "cpu_small");
  const render = presets.rows.find((view) => view.row.name === "render");
  assert.ok(cpu !== undefined && small !== undefined && render !== undefined);

  assert.equal(editRowName(classes, cpu), "Edit CPU heavy");
  assert.equal(resetRowName(classes, cpu), "Reset CPU heavy to its built-in size");
  assert.equal(rowRemovedText(classes, cpu), "CPU heavy reset to its built-in size.");
  assert.equal(rowRemovedText(presets, small), "Preset cpu_small reset to its built-in size.");
  assert.equal(deleteRowName(render), "Delete preset render");
  assert.equal(rowRemovedText(presets, render), "Preset render deleted.");
});

test("deleting a preset then Undo writes it back", async () => {
  const { session, client, timers } = await loaded({ state: renderState() });
  const presets = as(session.setting("presets"), SizeTableModel);
  const old = presets.value.find((row) => row.name === "render");
  assert.ok(old !== undefined);

  presets.remove("render");
  assert.ok(!presets.value.some((row) => row.name === "render"));
  await fire(timers);
  presets.restore(old);
  await fire(timers);

  assert.equal(client.patches.length, 2);
  assert.deepEqual(client.patches[0]?.changes, [
    { kind: "reset_row", table: "presets", name: "render" },
  ]);
  assert.deepEqual(client.patches[1]?.changes, [
    {
      kind: "set_row",
      table: "presets",
      name: "render",
      profile: old.profile,
      min_cores: old.min_cores,
    },
  ]);
});

test("a row proposal follows the dialog and waits for a valid row", async () => {
  const { session, client } = await loaded({ state: renderState() });
  const presets = as(session.setting("presets"), SizeTableModel);
  const draft = presets.edit("render");
  assert.ok(draft !== null);
  draft.field("cores")?.commit(10);

  const proposal = presets.proposal(draft);
  assert.equal(proposal?.length, 1);
  const [row] = proposal ?? [];
  assert.equal(row?.kind === "set_row" ? row.profile.cores : null, 10);

  draft.setName("render_big");
  assert.deepEqual(presets.proposal(draft)?.map(describe), [
    "reset_row render",
    "set_row render_big",
  ]);

  draft.setName("Render");
  assert.equal(draft.ready, false);
  assert.equal(presets.proposal(draft), null);

  const added = presets.add();
  added.setName("fresh");
  // A new preset needs its cores and memory first, and a proposal shows no issue.
  assert.equal(presets.proposal(added), null);
  assert.equal(added.field("cores")?.issue, null);
  assert.deepEqual(client.patches, []);
});

test("the preview adds the dialog's changes after the unsaved ones", async () => {
  const { session, client } = await loaded();
  as(session.setting("admission.headroom_pct"), NumberModel).commit(30);
  const classes = as(session.setting("classes"), SizeTableModel);
  const draft = classes.edit("mixed");
  assert.ok(draft !== null);
  draft.field("cores")?.commit(12);

  await session.preview(classes.proposal(draft) ?? []);

  assert.deepEqual(client.previews.at(-1)?.map(describe), ["set_number", "set_row mixed"]);
});

test("resetting print-only flags then Undo writes the old list back", async () => {
  const state = fixtureState((json) =>
    setValue(json, "classifier.light_flags", ["--help", "--dry-run"])
  );
  const { session, client, timers } = await loaded({ state });
  const flags = as(session.setting("classifier.light_flags"), StringListModel);
  assert.ok(flags.canReset);

  const undo = resetField(flags);
  await fire(timers);
  undo?.();
  await fire(timers);

  assert.equal(client.patches.length, 2);
  assert.deepEqual(client.patches[0]?.changes, [
    { kind: "reset", key: "classifier.light_flags" },
  ]);
  assert.deepEqual(client.patches[1]?.changes, [
    { kind: "set_list", key: "classifier.light_flags", value: ["--help", "--dry-run"] },
  ]);
});

test("deleting a rule then Undo writes the whole list back", async () => {
  const { session, client, timers } = await loaded();
  const rules = as(session.setting("classifier.rules"), RuleListModel);
  const old = rules.value.slice();

  rules.remove(0);
  assert.equal(rules.value[0]?.program, "pnpm");
  await fire(timers);
  rules.replace(old);
  await fire(timers);

  assert.deepEqual(client.patches.at(-1)?.changes, [{ kind: "set_rules", rules: old }]);
});

/**
 * @param {RuleListModel} rules
 * @param {import("../allot@mehran.tech/lib/setting-values.js").RuleDraft} draft
 * @returns {HeavyRule[]}
 */
function proposedRules(rules, draft) {
  const proposal = rules.proposal(draft);
  assert.equal(proposal?.length, 1);
  const [change] = proposal ?? [];
  assert.equal(change?.kind, "set_rules");
  return change?.kind === "set_rules" ? change.rules : [];
}

test("a rule proposal puts the draft in its place or at the end", async () => {
  const { session } = await loaded();
  const rules = as(session.setting("classifier.rules"), RuleListModel);
  const count = rules.value.length;

  const edited = rules.edit(1);
  assert.ok(edited !== null);
  edited.setProgram("bun");
  const replaced = proposedRules(rules, edited);
  assert.equal(replaced.length, count);
  assert.equal(replaced[1]?.program, "bun");

  const added = rules.add();
  // A new rule needs its program first, and a proposal shows no issue.
  assert.equal(rules.proposal(added), null);
  assert.equal(added.programIssue, null);
  added.setProgram("ninja");
  added.setClass("mixed");
  const appended = proposedRules(rules, added);
  assert.equal(appended.length, count + 1);
  assert.equal(appended.at(-1)?.class, "mixed");
});

test("rule list texts follow the list", async () => {
  const { session } = await loaded();
  const rules = as(session.setting("classifier.rules"), RuleListModel);
  const list = rules.value;
  const classLabel = classLabels(as(session.setting("classes"), SizeTableModel).control);
  const first = list[0];
  const third = list[2];
  assert.ok(first !== undefined && third !== undefined);

  assert.equal(rulesStatus(rules), BUILT_IN_RULES_TEXT);
  assert.equal(ruleArguments(first), "build · test · run · bench");
  assert.equal(ruleArguments(third), ANY_ARGUMENTS);
  assert.equal(
    ruleRowName(0, first, classLabel),
    "Rule 1: cargo, build · test · run · bench, CPU heavy",
  );
  assert.equal(ruleMenuName(2), "More options for rule 3");
  assert.equal(ruleDeletedText(2), "Rule 3 deleted.");
  const second = rules.edit(1);
  assert.ok(second !== null);
  assert.equal(ruleDialogTitle(second), "Edit rule 2");
  assert.equal(ruleDialogTitle(rules.add()), "Add rule");

  const state = fixtureState((json) =>
    setValue(json, "classifier.rules", [{ program: "ninja", args: [], class: "cpu_heavy" }])
  );
  const yours = (await loaded({ state })).session;
  assert.equal(rulesStatus(as(yours.setting("classifier.rules"), RuleListModel)), YOUR_RULES_TEXT);
  assert.deepEqual(useBuiltInConfirmation(5, 6), {
    title: "Use the built-in rules?",
    body: "Your 5 rules are replaced by the 6 built-in ones. This can't be undone.",
    button: "Use Built-in Rules",
  });
  assert.equal(
    useBuiltInConfirmation(1, 6).body,
    "Your 1 rule is replaced by the 6 built-in ones. This can't be undone.",
  );
});

test("the fixture test reads one line per command", async () => {
  const { session } = await loaded();
  const rules = as(session.setting("classifier.rules"), RuleListModel);
  const classLabel = classLabels(as(session.setting("classes"), SizeTableModel).control);
  const result = decodeCommandTest(JSON.parse(fixtureText("command-test.json")));

  assert.deepEqual(testLines(result, rules.value, classLabel, english), [
    "Heavy: needs a CPU heavy lease (8 cores, 4 GB memory). Rule 1: cargo with build, test, " +
      "run or bench.",
    "Print-only: --help only prints and exits, so no lease is needed.",
    "Not heavy: no rule matches cargo fmt.",
  ]);
  assert.deepEqual(matchedRules(result), [0]);
  assert.deepEqual(matchedRules(null), []);
});

/**
 * @param {string} jobClass
 * @param {[number, number, number, number]} profile
 */
function size(jobClass, [cores, ram_mb, gpu_pct, vram_mb]) {
  return {
    class: jobClass,
    min_cores: 1,
    profile: { cores, ram_mb, gpu_pct, vram_mb, est_secs: 300 },
  };
}

test("two heavy commands name the combined lease", () => {
  /** @type {HeavyRule[]} */
  const rules = [
    { program: "cargo", args: [], class: "cpu_heavy" },
    { program: "python*", args: ["train*.py"], class: "gpu_heavy" },
  ];
  const result = decodeCommandTest({
    lease: size("mixed", [6, 8192, 50, 2048]),
    commands: [
      {
        command: "cargo build",
        kind: "heavy",
        rule_index: 0,
        size: size("cpu_heavy", [8, 4096, 0, 0]),
      },
      {
        command: "python train.py",
        kind: "heavy",
        rule_index: 1,
        size: size("gpu_heavy", [2, 4096, 50, 2048]),
      },
    ],
  });

  assert.deepEqual(testLines(result, rules, labels, english), [
    "Heavy: needs a CPU heavy lease (8 cores, 4 GB memory). Rule 1: cargo with any arguments.",
    "Heavy: needs a GPU heavy lease (2 cores, 4 GB memory, 50 % of the GPU, 2 GB video " +
      "memory). Rule 2: python* with train*.py.",
    "The whole line needs one Mixed lease (6 cores, 8 GB memory, 50 % of the GPU, 2 GB video " +
      "memory).",
  ]);
  assert.deepEqual(matchedRules(result), [0, 1]);
});

test("a line with no command says so", () => {
  const result = decodeCommandTest({ commands: [], lease: null });

  assert.deepEqual(testLines(result, [], labels, english), [NO_COMMAND_TEXT]);
});
