import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DurationModel,
  MemoryModel,
  NumberModel,
  RuleListModel,
  SizeTableModel,
  StringListModel,
  SwitchModel,
  issueText,
  quantityText,
} from "../allot@mehran.tech/lib/setting-values.js";
import {
  REFRESH_INTERVAL_MS,
  SAVE_DELAY_MS,
  parseStopRecord,
} from "../allot@mehran.tech/lib/settings-session.js";
import {
  as,
  failure,
  fire,
  fixtureState,
  loaded,
  withRender,
} from "./settings-support.mjs";
import { fixtureText } from "./support.mjs";

/**
 * @import { SettingsState } from "../allot@mehran.tech/contract.g.mjs"
 * @import { SettingsReply } from "../allot@mehran.tech/lib/results.js"
 * @import { SettingsSession } from "../allot@mehran.tech/lib/settings-session.js"
 */

/** The fixture's state with a preset of the user's own, `render`. */
const stateWithRender = () => fixtureState(withRender);

/** @param {number} value */
const plain = (value) => String(value);

test("the fixture loads every setting with its view model", async () => {
  const { session } = await loaded();

  assert.equal(session.status, "ready");
  assert.equal(session.readOnly, false);
  assert.deepEqual(session.pendingRestart, ["daemon.port"]);
  assert.deepEqual(
    session.pages.map((page) => page.spec.id),
    ["admission", "sizes", "commands", "benchmarks", "leases", "service"],
  );
  as(session.setting("estimates.lower"), SwitchModel);
  as(session.setting("admission.headroom_pct"), NumberModel);
  as(session.setting("leases.idle_release_secs"), DurationModel);
  as(session.setting("classifier.light_flags"), StringListModel);
  as(session.setting("presets"), SizeTableModel);
  as(session.setting("classifier.rules"), RuleListModel);
  assert.deepEqual(session.saveStatus, { kind: "saved" });
});

test("the Linux group shows only on Linux", async () => {
  const linux = await loaded({ platform: "linux" });
  const windows = await loaded({ platform: "windows" });

  const groups = (/** @type {SettingsSession} */ session) =>
    session.pages.find((page) => page.spec.id === "service")?.groups.map((group) => group.spec.id);
  assert.deepEqual(groups(linux.session), ["connection", "linux"]);
  assert.deepEqual(groups(windows.session), ["connection"]);
  assert.equal(windows.session.setting("linux.scopes"), undefined);
});

test("reset shows only for values that differ from the default", async () => {
  const { session } = await loaded();

  assert.equal(session.setting("daemon.port")?.canReset, true);
  assert.equal(session.setting("admission.headroom_pct")?.canReset, false);
  assert.equal(session.setting("classes")?.canReset, true);
  assert.equal(session.setting("presets")?.canReset, false);
  assert.deepEqual(
    session.changedOn("service").map((model) => model.key),
    ["daemon.port"],
  );
});

test("single values save as one burst 600 ms after the last change", async () => {
  const { session, client, timers } = await loaded();
  const lower = as(session.setting("estimates.lower"), SwitchModel);
  const headroom = as(session.setting("admission.headroom_pct"), NumberModel);

  lower.set(false);
  headroom.commit(20);
  headroom.commit(25);

  assert.deepEqual(timers.delays, [SAVE_DELAY_MS]);
  assert.deepEqual(session.saveStatus, { kind: "unsaved", count: 2 });
  await fire(timers);

  assert.deepEqual(client.patches, [
    {
      revision: "69fd99abe4f654c7",
      changes: [
        { kind: "set_switch", key: "estimates.lower", value: false },
        { kind: "set_number", key: "admission.headroom_pct", value: 25 },
      ],
    },
  ]);
  assert.deepEqual(session.saveStatus, { kind: "saved" });
  assert.equal(headroom.dirty, false);
});

test("a value outside its range shows why and is not sent", async () => {
  const { session, client, timers } = await loaded();
  const headroom = as(session.setting("admission.headroom_pct"), NumberModel);

  headroom.commit(100);

  assert.deepEqual(timers.delays, []);
  assert.equal(headroom.value, 100);
  assert.equal(issueText(headroom.issue ?? { kind: "blank" }, plain),
    "Enter a number at least 0 and below 100.");
  assert.deepEqual(session.saveStatus, { kind: "unsaved", count: 1 });
  headroom.undo();
  assert.equal(headroom.value, 15);
  assert.deepEqual(session.saveStatus, { kind: "saved" });
  assert.equal(client.patches.length, 0);
});

test("a value below the setting it depends on names that setting", async () => {
  const { session } = await loaded();
  const wait = as(session.setting("benchmark.max_quiet_wait_secs"), DurationModel);

  wait.setUnit("seconds");
  wait.commit(5);

  assert.deepEqual(wait.issue, { kind: "at_least", label: "Quiet for", value: 10,
    quantity: "seconds" });
  assert.equal(issueText(wait.issue ?? { kind: "blank" }, plain),
    "Must be at least Quiet for (10 seconds).");
});

test("allotd's refusal lands under its field and the rest is sent again", async () => {
  const { session, client, timers } = await loaded();
  const problem = { key: "admission.headroom_pct", message: "must be at least 0 and below 100" };
  client.writes.push(failure("invalid", { problems: [problem] }));
  as(session.setting("admission.headroom_pct"), NumberModel).commit(50);
  as(session.setting("estimates.lower"), SwitchModel).set(false);

  await fire(timers);
  await fire(timers);

  const headroom = session.setting("admission.headroom_pct");
  assert.deepEqual(headroom?.issue, {
    kind: "service",
    message: "Headroom must be at least 0 and below 100",
  });
  assert.equal(headroom?.value, 50);
  assert.deepEqual(client.patches[1]?.changes, [
    { kind: "set_switch", key: "estimates.lower", value: false },
  ]);
  assert.deepEqual(session.saveStatus, { kind: "unsaved", count: 1 });
});

test("a hand edit in between shows the file's values and can apply the change again", async () => {
  const { session, client, timers } = await loaded();
  const margin = as(session.setting("estimates.margin_pct"), NumberModel);
  client.writes.push(failure("conflict", { revision: "f38fd8b70ab492ce" }));
  const edited = { ...client.state, revision: "f38fd8b70ab492ce" };
  client.reads.push({ ok: true, value: edited });

  margin.commit(15);
  await fire(timers);

  assert.equal(margin.value, 10);
  assert.deepEqual(session.conflict?.edits.map((edit) => edit.changes), [
    [{ kind: "set_number", key: "estimates.margin_pct", value: 15 }],
  ]);
  session.applyMine();
  await fire(timers);
  assert.equal(session.conflict, null);
  assert.deepEqual(client.patches[1], {
    revision: "f38fd8b70ab492ce",
    changes: [{ kind: "set_number", key: "estimates.margin_pct", value: 15 }],
  });
});

test("a held file keeps the changes unsaved until the next save", async () => {
  const { session, client, timers } = await loaded();
  client.writes.push(failure("held", { message: "config.toml is open in another program" }));

  as(session.setting("hook.fail_open"), SwitchModel).set(false);
  await fire(timers);

  assert.equal(session.writeProblem, "config.toml is open in another program");
  assert.deepEqual(session.saveStatus, { kind: "unsaved", count: 1 });
  await fire(timers);
  assert.equal(client.patches.length, 2);
  assert.equal(session.writeProblem, null);
});

test("a rejected file makes the pages read-only", async () => {
  const { session, client } = await loaded();
  client.reads.push({ ok: true, value: { ...client.state, file_error: "expected `=`" } });

  await session.refresh();

  assert.equal(session.readOnly, true);
  assert.equal(session.fileError, "expected `=`");
});

test("no answer reads the stop record to tell a stop from a failure", async () => {
  const record = fixtureText("stop-record.json");
  const stopped = await loaded({ stopRecord: record });
  const down = await loaded();
  for (const { session, client } of [stopped, down]) {
    client.reads.push(failure("unreachable", { message: "Connection refused" }));
    await session.refresh();
  }

  assert.equal(stopped.session.status, "stopped");
  assert.equal(stopped.session.stopRecord?.by, "panel");
  assert.equal(down.session.status, "down");
  assert.equal(down.session.detail, "Connection refused");
  assert.equal(down.session.readOnly, true);
  assert.equal(down.session.setting("daemon.port")?.value, 47999);
});

test("a stop record that is not one still counts as a stop", () => {
  assert.deepEqual(parseStopRecord("not json"), { by: "cli", stopped_at_ms: 0 });
  assert.equal(parseStopRecord(null), null);
});

test("showing the window fetches now and every 5 s; hiding it saves", async () => {
  const { session, client, timers } = await loaded();

  session.setVisible(true);
  assert.deepEqual(timers.delays, [REFRESH_INTERVAL_MS]);
  as(session.setting("hook.fail_open"), SwitchModel).set(false);
  session.setVisible(false);
  await fire(timers);

  assert.equal(client.patches.length, 1);
  assert.deepEqual(timers.delays, []);
});

test("durations show their largest whole unit and save seconds", async () => {
  const { session, client, timers } = await loaded();
  const idle = as(session.setting("leases.idle_release_secs"), DurationModel);

  assert.equal(idle.unit.name, "minutes");
  assert.equal(idle.shown, 3);
  idle.setUnit("seconds");
  assert.equal(idle.shown, 180);
  idle.setUnit("minutes");
  idle.commit(5);
  assert.equal(idle.value, 300);
  idle.setOn(false);
  assert.equal(idle.isOn, false);
  idle.setOn(true);
  await fire(timers);

  assert.deepEqual(client.patches[0]?.changes, [
    { kind: "set_number", key: "leases.idle_release_secs", value: 300 },
  ]);
});

test("turning a duration off writes its off value", async () => {
  const { session, client, timers } = await loaded();

  as(session.setting("leases.idle_release_secs"), DurationModel).setOn(false);
  await fire(timers);

  assert.deepEqual(client.patches[0]?.changes, [
    { kind: "set_number", key: "leases.idle_release_secs", value: 0 },
  ]);
});

test("chips save on add and remove, never blank or twice", async () => {
  const { session, client, timers } = await loaded();
  const flags = as(session.setting("classifier.light_flags"), StringListModel);

  assert.equal(flags.add("  "), false);
  assert.deepEqual(flags.issue, { kind: "blank" });
  assert.equal(flags.add("--help"), false);
  assert.deepEqual(flags.issue, { kind: "duplicate" });
  assert.equal(flags.add(" --dry-run "), true);
  await fire(timers);

  assert.deepEqual(client.patches[0]?.changes, [
    {
      kind: "set_list",
      key: "classifier.light_flags",
      value: ["--help", "-h", "--version", "-V", "/?", "--dry-run"],
    },
  ]);
});

test("a new preset needs a free name and its sizes, then saves at once", async () => {
  const { session, client, timers } = await loaded();
  const presets = as(session.setting("presets"), SizeTableModel);
  const draft = presets.add();

  assert.equal(draft.canRename, true);
  draft.setName("Render");
  assert.deepEqual(draft.nameIssue, { kind: "pattern" });
  draft.setName("light");
  assert.deepEqual(draft.nameIssue, { kind: "name_taken" });
  draft.setName("render");
  assert.equal(presets.save(draft), false);
  assert.deepEqual(draft.field("cores")?.issue, { kind: "required" });
  assert.equal(draft.field("min_cores")?.issue, null);

  draft.field("cores")?.commit(6);
  const ram = as(draft.field("ram_mb"), MemoryModel);
  ram.setUnit("GB");
  ram.commit(1.5);
  assert.equal(ram.value, 1536);
  assert.equal(presets.save(draft), true);
  await fire(timers);

  assert.deepEqual(client.patches[0]?.changes, [
    {
      kind: "set_row",
      table: "presets",
      name: "render",
      profile: { cores: 6, ram_mb: 1536, gpu_pct: 0, vram_mb: 0, est_secs: 300 },
      min_cores: null,
    },
  ]);
});

test("a minimum above the cores is refused in the dialog", async () => {
  const { session } = await loaded();
  const classes = as(session.setting("classes"), SizeTableModel);
  const draft = classes.edit("mixed");

  assert.ok(draft !== null);
  assert.equal(draft.canRename, false);
  draft.field("min_cores")?.commit(8);

  assert.deepEqual(draft.field("min_cores")?.issue, {
    kind: "at_most",
    label: "Cores",
    value: 6,
    quantity: "cores",
  });
  assert.equal(classes.save(draft), false);
  draft.field("cores")?.commit(8);
  assert.equal(draft.field("min_cores")?.issue, null);
});

test("renaming a preset removes the old row in the same write", async () => {
  const { session, client, timers } = await loaded({ state: stateWithRender() });
  const presets = as(session.setting("presets"), SizeTableModel);
  const draft = presets.edit("render");

  assert.ok(draft !== null);
  assert.equal(as(draft.field("ram_mb"), MemoryModel).unit.name, "GB");
  draft.setName("render_big");
  assert.equal(presets.save(draft), true);
  assert.deepEqual(
    presets.value.map((row) => row.name).filter((name) => name.startsWith("render")),
    ["render_big"],
  );
  await fire(timers);

  assert.deepEqual(
    client.patches[0]?.changes.map((change) => [change.kind, "name" in change && change.name]),
    [["reset_row", "render"], ["set_row", "render_big"]],
  );
});

test("deleting a user preset and resetting a built-in one are row resets", async () => {
  const { session, client, timers } = await loaded({ state: stateWithRender() });
  const presets = as(session.setting("presets"), SizeTableModel);

  assert.equal(presets.rows.find((row) => row.row.name === "render")?.builtIn, false);
  presets.remove("render");
  await fire(timers);

  assert.deepEqual(client.patches[0]?.changes, [
    { kind: "reset_row", table: "presets", name: "render" },
  ]);
  assert.equal(presets.describeProblem("presets.render.min_cores", "must be above 0"),
    "render: Minimum cores must be above 0");
});

test("rules are written whole on every edit, move and delete", async () => {
  const { session, client, timers } = await loaded();
  const rules = as(session.setting("classifier.rules"), RuleListModel);
  const programs = () => rules.value.map((rule) => rule.program);

  rules.move(0, 2);
  assert.deepEqual(programs(), ["pnpm", "msbuild", "cargo", "cmake", "python*", "py"]);
  rules.remove(5);
  const draft = rules.edit(1);
  assert.ok(draft !== null);
  draft.setProgram(" ");
  assert.equal(rules.save(draft), false);
  assert.deepEqual(draft.programIssue, { kind: "blank" });
  draft.setProgram("ninja");
  assert.equal(draft.args.add("-j"), true);
  draft.setClass("mixed");
  assert.equal(rules.save(draft), true);
  await fire(timers);

  const last = client.patches[client.patches.length - 1];
  assert.deepEqual(last?.changes, [
    {
      kind: "set_rules",
      rules: [
        { program: "pnpm", args: ["build*", "test*"], class: "cpu_heavy" },
        { program: "ninja", args: ["-j"], class: "mixed" },
        { program: "cargo", args: ["build", "test", "run", "bench"], class: "cpu_heavy" },
        { program: "cmake", args: ["--build"], class: "cpu_heavy" },
        { program: "python*", args: ["train*.py"], class: "gpu_heavy" },
      ],
    },
  ]);
  assert.equal(rules.describeProblem("classifier.rules[2].program", "must not be blank"),
    "Rule 3: Program must not be blank");
});

test("Reset page removes every changed key on it in one write", async () => {
  const { session, client, timers } = await loaded();

  session.resetChanged("service");
  await fire(timers);

  assert.deepEqual(client.patches[0]?.changes, [{ kind: "reset", key: "daemon.port" }]);
  assert.equal(session.setting("daemon.port")?.value, 47999);
});

test("a preview sends the unsaved changes", async () => {
  const { session, client } = await loaded();
  as(session.setting("admission.headroom_pct"), NumberModel).commit(30);

  await session.preview();

  assert.deepEqual(client.previews, [
    [{ kind: "set_number", key: "admission.headroom_pct", value: 30 }],
  ]);
});

test("values read with their unit", () => {
  assert.equal(quantityText(15, "percent", plain), "15 %");
  assert.equal(quantityText(180, "seconds", plain), "3 minutes");
  assert.equal(quantityText(1, "seconds", plain), "1 second");
  assert.equal(quantityText(2048, "megabytes", plain), "2 GB");
  assert.equal(quantityText(1000, "megabytes", plain), "1000 MB");
  assert.equal(quantityText(1, "cores", plain), "1 core");
  assert.equal(quantityText(47823, "port", (value) => value.toLocaleString("en")), "47823");
  assert.equal(
    issueText({ kind: "range", min: { value: 1, exclusive: false },
      max: { value: 65535, exclusive: false } }, plain),
    "Enter a number from 1 to 65535.",
  );
  assert.equal(
    issueText({ kind: "range", min: { value: 0, exclusive: true }, max: null }, plain),
    "Enter a number above 0.",
  );
});

test("a failed save other than a held file waits for Try again or the next edit", async () => {
  const { session, client, timers } = await loaded();
  client.writes.push(failure("failed", { message: "config.toml is read-only" }));

  as(session.setting("hook.fail_open"), SwitchModel).set(false);
  await fire(timers);
  await fire(timers);

  assert.equal(session.writeProblem, "config.toml is read-only");
  assert.equal(client.patches.length, 1);
  assert.equal(timers.pending.size, 0);
  assert.deepEqual(session.saveStatus, { kind: "unsaved", count: 1 });

  client.writes.push(failure("unavailable", { message: "the service is starting" }));
  await session.flush();
  await fire(timers);
  assert.equal(client.patches.length, 2);
  assert.equal(timers.pending.size, 0);

  as(session.setting("estimates.lower"), SwitchModel).set(false);
  await fire(timers);
  assert.equal(client.patches.length, 3);
  assert.equal(client.patches[2]?.changes.length, 2);
  assert.equal(session.writeProblem, null);
});

test("changes allotd did not answer go out again once a refresh finds it back", async () => {
  const { session, client, timers } = await loaded();
  client.writes.push(failure("unreachable", { message: "Connection refused" }));

  as(session.setting("hook.fail_open"), SwitchModel).set(false);
  await fire(timers);
  await fire(timers);
  assert.equal(client.patches.length, 1);
  assert.deepEqual(session.saveStatus, { kind: "unsaved", count: 1 });

  await session.refresh();
  await fire(timers);
  assert.equal(client.patches.length, 2);
  assert.deepEqual(session.saveStatus, { kind: "saved" });
});

test("a refresh answered before a save finished does not bring back the old revision", async () => {
  const { session, client } = await loaded();
  const old = client.state;
  /** @type {(reply: SettingsReply<SettingsState>) => void} */
  let answer = () => {};
  client.settings = () => new Promise((resolve) => (answer = resolve));
  const slow = session.refresh();
  client.writes.push({ ok: true, value: { ...old, revision: "0123456789abcdef" } });

  as(session.setting("hook.fail_open"), SwitchModel).set(false);
  await session.flush();
  answer({ ok: true, value: old });
  await slow;
  as(session.setting("estimates.lower"), SwitchModel).set(false);
  await session.flush();

  assert.equal(client.patches[1]?.revision, "0123456789abcdef");
});

test("a 422 no edit owns shows allotd's message and undoes the edits", async () => {
  const { session, client, timers } = await loaded();
  client.writes.push(
    failure("invalid", {
      message: "the limits do not fit together",
      problems: [{ key: "somewhere.else", message: "must be larger" }],
    }),
  );
  const fail = as(session.setting("hook.fail_open"), SwitchModel);
  const before = fail.value;

  fail.set(!before);
  await fire(timers);

  assert.equal(session.writeProblem, "the limits do not fit together");
  assert.equal(fail.value, before);
  assert.deepEqual(session.saveStatus, { kind: "saved" });
  await session.flush();
  assert.equal(session.writeProblem, null);
});

test("a blank or duplicate chip is not an unsaved change and clears with the text", async () => {
  const { session } = await loaded();
  const flags = as(session.setting("classifier.light_flags"), StringListModel);

  assert.equal(flags.add(" "), false);
  assert.deepEqual(session.saveStatus, { kind: "saved" });
  assert.equal(flags.entryChanged(), true);
  assert.equal(flags.issue, null);
  assert.equal(flags.entryChanged(), false);

  assert.equal(flags.add("--help"), false);
  assert.equal(flags.entryChanged(), true);
  assert.equal(flags.issue, null);
});

test("a number that is not finite gets the number error", async () => {
  const { session } = await loaded();
  const headroom = as(session.setting("admission.headroom_pct"), NumberModel);
  const wait = as(session.setting("benchmark.max_quiet_wait_secs"), DurationModel);

  headroom.commit(Number("9".repeat(400)));
  assert.deepEqual(headroom.issue, { kind: "not_a_number" });
  headroom.commit(Number.NaN);
  assert.deepEqual(headroom.issue, { kind: "not_a_number" });
  wait.setUnit("hours");
  wait.commit(1e306);
  assert.deepEqual(wait.issue, { kind: "not_a_number" });
});

test("no label, help or intro in the spec holds markup characters", () => {
  const texts = [];
  for (const page of fixtureState().spec.pages) {
    texts.push(["page", page.label], ["page intro", page.intro ?? ""]);
    for (const group of page.groups) {
      texts.push(["group", group.label]);
      for (const setting of group.settings) {
        texts.push([setting.key, setting.label], [setting.key, setting.help]);
      }
    }
  }

  assert.ok(texts.length > 20);
  for (const [name, text] of texts) {
    assert.doesNotMatch(text, /[&<]/, `${name}: ${text}`);
  }
});
