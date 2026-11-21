// The Settings window's shared model and editors, which the GNOME preferences and the KDE
// widget's window both build on.

import assert from "node:assert/strict";
import { test } from "node:test";

import { dataPaths } from "../allot@mehran.tech/lib/config.js";
import { DRAFT_NOT_READY_TEXT } from "../allot@mehran.tech/lib/rules-text.js";
import { SERVICE_STOPPED_TEXT } from "../allot@mehran.tech/lib/live-text.js";
import { STOP_CONFIRM_TITLE } from "../allot@mehran.tech/lib/service-menu.js";
import {
  NumberModel,
  RuleListModel,
  SizeTableModel,
  StringListModel,
} from "../allot@mehran.tech/lib/setting-values.js";
import {
  deleteRule,
  draftRules,
  moveRule,
  numberEnabled,
  removeSizeRow,
  resettableRow,
  ruleRowItems,
  savedRules,
  settingNotes,
  sizeRowItems,
  testCommand,
} from "../allot@mehran.tech/lib/settings-editors.js";
import { LOADING_TEXT } from "../allot@mehran.tech/lib/settings-text.js";
import {
  ALLOTD_PAGES,
  SettingsWindowModel,
  pageSections,
} from "../allot@mehran.tech/lib/settings-window.js";
import { FakeSettings, as, english, failure, settleAll } from "./settings-support.mjs";
import { FakeTimers, fixtureState } from "./support.mjs";

/** @import { Reach } from "../allot@mehran.tech/lib/service-row.js" */

/** @type {Reach} */
const PORT_REACH = { kind: "port", port: 47823 };
/** @type {Reach} */
const BUS_REACH = { kind: "bus" };

/**
 * @import { PollResult } from "../allot@mehran.tech/lib/results.js"
 * @import { Confirmation } from "../allot@mehran.tech/lib/settings-text.js"
 * @import { PageSection } from "../allot@mehran.tech/lib/settings-window.js"
 */

/** The settings client plus what the window's service row and restart use. */
class FakeDaemon extends FakeSettings {
  /** @type {Array<boolean | undefined>} */
  shutdowns = [];
  /** @type {number[]} */
  ports = [];
  /** @type {PollResult} */
  pollAnswer = { status: "stopped", state: null, version: null, detail: null };
  version = /** @type {string | null} */ ("0.6.0");

  poll = async () => this.pollAnswer;

  /** @param {{ record: boolean }} [options] */
  shutdown = async (options) => {
    this.shutdowns.push(options?.record);
    return null;
  };

  healthVersion = async () => this.version;
}

/**
 * A window model on fakes, loaded once from the settings-state fixture.
 *
 * @param {{ load: boolean, reach?: Reach }} [options]
 */
async function windowModel(options = { load: true, reach: PORT_REACH }) {
  const client = new FakeDaemon();
  const timers = new FakeTimers();
  /** @type {Array<{ confirmation: Confirmation, onConfirm: () => void }>} */
  const asked = [];
  /** @type {string[]} */
  const notices = [];
  /** @type {string[]} */
  const opened = [];
  /** @type {string[]} */
  const logged = [];
  /** @type {string[]} */
  const calls = [];
  /** @type {string[]} */
  const missing = [];
  let clock = 0;
  const model = new SettingsWindowModel({
    client,
    timers,
    launcher: {
      start: async () => {
        calls.push("launcher.start");
        return null;
      },
      restartThroughUnit: async () => false,
      restartUnit: async () => {
        calls.push("launcher.restartUnit");
        return null;
      },
    },
    paths: dataPaths("/home/u"),
    reach: options.reach ?? PORT_REACH,
    followPort: (port) => client.ports.push(port),
    number: english,
    readStopRecord: async () => null,
    host: {
      confirm: (confirmation, onConfirm) => asked.push({ confirmation, onConfirm }),
      notify: (text) => notices.push(text),
      log: (text) => logged.push(text),
      openPath: (path) => opened.push(path),
      exists: async (path) => !missing.includes(path),
      now: () => clock,
    },
    stop: async () => {
      calls.push("stop");
      return null;
    },
  });
  let changes = 0;
  model.onChange = () => {
    changes += 1;
  };
  if (options.load) {
    await model.refresh();
    await settleAll();
  }
  return {
    model,
    client,
    timers,
    asked,
    notices,
    logged,
    opened,
    missing,
    calls,
    changes: () => changes,
    advance: (/** @type {number} */ ms) => {
      clock += ms;
    },
  };
}

/** @param {PageSection} section */
function sectionName(section) {
  switch (section.kind) {
    case "extra":
      return section.id;
    case "reset":
      return section.all ? "reset all" : "reset";
    case "settings":
      return section.live === null ? section.title : `${section.title} (${section.live})`;
    default:
      return `${section.kind}: ${section.title}`;
  }
}

test("pages list their parts in the design's order, advanced keys last", async () => {
  const { model } = await windowModel();
  const sections = Object.fromEntries(
    model.session.pages.map((page) => [page.spec.id, pageSections(page).map(sectionName)]),
  );

  assert.deepEqual(sections, {
    admission: [
      "Keep free for your desktop (keep_free)",
      "Learn from past runs (learn)",
      "what_fits",
      "reset",
    ],
    sizes: ["sizes: Classes", "sizes: Presets", "reset"],
    commands: ["rules: Heavy-command rules", "Print-only flags (print_only)", "reset"],
    benchmarks: ["Quiet machine (quiet)", "reset"],
    leases: ["Leases (leases)", "Advanced: History (history)", "reset"],
    service: [
      "service_status",
      "Connection",
      "Linux (linux)",
      "files",
      "Advanced: Connection (connection)",
      "reset all",
    ],
  });
  assert.deepEqual(
    ALLOTD_PAGES.map((page) => page.id),
    model.session.pages.map((page) => page.spec.id),
  );
});

test("a page reads Loading settings… until allotd answers, then the save status", async () => {
  const { model } = await windowModel({ load: false });

  assert.equal(model.pageState("admission").status, LOADING_TEXT);
  assert.equal(model.serviceRow, null);

  await model.refresh();
  await settleAll();

  assert.deepEqual(model.pageState("admission"), {
    banners: [],
    status: "All changes saved",
    readOnly: false,
  });
});

test("groups with numbers have live sentences, the rest none", async () => {
  const { model } = await windowModel();

  for (const [page, group] of [
    ["admission", "learn"],
    ["benchmarks", "quiet"],
    ["leases", "history"],
  ]) {
    const sentence = model.live(page ?? "", group ?? "");
    assert.ok(sentence !== null && sentence.length > 0, `${page}/${group}`);
  }
  assert.equal(model.live("service", "linux"), null);
});

test("What Fits waits for a preview, and says so", async () => {
  const { model } = await windowModel();

  model.showPage("admission");

  assert.deepEqual(model.whatFits.rows, []);
  assert.ok(model.whatFits.caption.length > 0);
});

test("the Service row shows the version, and Stop goes ahead while idle", async () => {
  const { model, calls, asked } = await windowModel();

  assert.equal(model.serviceRow?.text, "Running · version 0.6.0 · port 47823");
  assert.equal(model.serviceRow?.button, "Stop Service…");

  model.pressService();
  await settleAll();

  assert.deepEqual(calls, ["stop"]);
  assert.equal(asked.length, 0);
});

test("Stop asks first while leases are active", async () => {
  const { model, client, calls, asked } = await windowModel();
  const state = fixtureState("busy");
  client.pollAnswer = { status: "running", state, version: "0.6.0", detail: null };

  model.pressService();
  await settleAll();

  assert.equal(asked[0]?.confirmation.title, STOP_CONFIRM_TITLE);
  assert.deepEqual(calls, []);
  asked[0]?.onConfirm();
  await settleAll();
  assert.deepEqual(calls, ["stop"]);
});

/** A window model whose allotd does not answer yet. */
async function downModel() {
  const window = await windowModel({ load: false });
  window.client.reads.push(failure("unreachable"));
  await window.model.refresh();
  await settleAll();
  return window;
}

test("Start Service shows Starting… until allotd answers", async () => {
  const { model, timers, calls } = await downModel();

  model.act("start_service");
  await settleAll();

  assert.deepEqual(calls, ["launcher.start"]);
  assert.equal(model.serviceRow?.button, "Starting…");
  assert.equal(model.serviceRow?.canPress, false);

  timers.fireAll();
  await settleAll();
  assert.equal(model.serviceRow?.button, "Stop Service…");
});

test("a start that times out offers Start Service again", async () => {
  const { model, timers, advance } = await downModel();

  model.act("start_service");
  await settleAll();
  advance(60_000);
  timers.fireAll();
  await settleAll();

  assert.equal(model.serviceRow?.button, "Start Service");
});

test("Restart Service moves the client to the saved port", async () => {
  const { model, client, timers } = await windowModel();
  const port = as(model.session.setting("daemon.port"), NumberModel);
  client.version = null;

  model.act("restart_service");
  await settleAll();
  timers.fireAll();
  await settleAll();

  assert.equal(port.saved, 47999);
  assert.deepEqual(client.shutdowns, [false]);
  assert.deepEqual(client.ports, [47999]);
  assert.deepEqual(model.reach, { kind: "port", port: 47999 });
});

test("Restart Service on the bus follows no port", async () => {
  const { model, client, timers } = await windowModel({ load: true, reach: BUS_REACH });
  client.version = null;

  model.act("restart_service");
  await settleAll();
  timers.fireAll();
  await settleAll();

  assert.deepEqual(client.shutdowns, [false]);
  assert.deepEqual(client.ports, []);
  assert.deepEqual(model.reach, { kind: "bus" });
  assert.equal(model.serviceRow?.text.includes("port"), false);
});

test("the bus panel's Service row has no port, and its window says it is off the bus", async () => {
  const { model, client } = await windowModel({ load: true, reach: BUS_REACH });

  assert.equal(model.serviceRow?.text, "Running · version 0.6.0");

  client.reads.push(failure("unreachable"));
  await model.refresh();
  await settleAll();
  model.showPage("service");

  assert.equal(model.serviceRow?.text, "Not on the session bus.");
  assert.equal(
    model.pageState("service").banners[0]?.message,
    "The service isn't on the session bus. Trying again every 5 s.",
  );
});

test("an allotd off the bus shows its own row and banner, and Restart goes through the unit", async () => {
  const { model, client, calls, timers } = await windowModel({ load: true, reach: BUS_REACH });

  client.reads.push(failure("off_bus"));
  await model.refresh();
  await settleAll();
  model.showPage("service");

  assert.equal(
    model.serviceRow?.text,
    "Runs without the session bus, so this panel cannot reach it. Restart it outside the " +
      "sandbox.",
  );
  assert.equal(model.serviceRow?.button, "Restart Service");
  assert.equal(model.serviceRow?.canPress, true);
  assert.deepEqual(model.pageState("service").banners, [
    {
      severity: "warning",
      message:
        "The service runs without the session bus, so Settings can't reach it. Restart it " +
        "outside the sandbox.",
      actions: ["restart_service", "open_logs"],
    },
  ]);

  model.pressService();
  await settleAll();
  timers.fireAll();
  await settleAll();

  assert.deepEqual(calls, ["launcher.restartUnit"]);
  assert.deepEqual(client.shutdowns, []);
});

test("a window that closes while Start Service waits sets no timer and shows no notice", async () => {
  const { model, timers, notices } = await windowModel();

  model.act("start_service");
  await settleAll();
  assert.ok(timers.pending.size > 0);
  model.dispose();
  assert.equal(timers.pending.size, 0);
  timers.fireAll();
  await settleAll();
  assert.equal(timers.pending.size, 0);
  assert.deepEqual(notices, []);
});

test("Reset Page and Reset All ask first, and only with something to reset", async () => {
  const { model, asked } = await windowModel();

  model.resetPage("benchmarks", "Benchmarks");
  assert.equal(asked.length, 0);

  model.resetPage("service", "Service");
  model.resetAll();

  assert.equal(asked.length, 2);
  assert.equal(model.canResetAll, true);
  assert.equal(asked[0]?.confirmation.button, "Reset 1 Setting");
  assert.equal(asked[1]?.confirmation.button, "Reset 2 Settings");
});

test("Open File and Open Logs open the panel's paths", async () => {
  const { model, opened } = await windowModel();

  model.act("open_file");
  await settleAll();
  model.openLogs();

  assert.deepEqual(opened, ["/home/u/.allot/config.toml", "/home/u/.allot/logs"]);
});

test("Open File on a config.toml that is not there yet says when it appears", async () => {
  const { model, opened, missing, notices } = await windowModel();
  missing.push("/home/u/.allot/config.toml");

  await model.openFile();

  assert.deepEqual(opened, []);
  assert.deepEqual(notices, ["config.toml does not exist yet. It appears once a setting is saved."]);
});

test("disposing stops the changes", async () => {
  const { model, changes } = await windowModel();
  const before = changes();

  model.dispose();
  await model.session.refresh().catch(() => {});

  assert.equal(changes(), before);
});

test("a setting's notes leave out empty lines", async () => {
  const { model } = await windowModel();
  const quiet = as(model.session.setting("benchmark.quiet_secs"), NumberModel);

  assert.deepEqual(settingNotes(quiet, english), []);
  quiet.commit(-1);
  assert.equal(settingNotes(quiet, english, "Restart needed.").length, 2);
  assert.equal(numberEnabled(quiet, false), true);
  assert.equal(numberEnabled(quiet, true), false);
});

test("size rows name their buttons; removing a row has an Undo", async () => {
  const { model } = await windowModel();
  const classes = as(model.session.setting("classes"), SizeTableModel);
  const presets = as(model.session.setting("presets"), SizeTableModel);

  const items = sizeRowItems(classes, model.fits, english);
  assert.equal(items.length, classes.rows.length);
  assert.equal(items[0]?.keyText, items[0]?.name);
  assert.equal(items[0]?.deleteName, null);
  assert.ok(sizeRowItems(presets, model.fits, english).every((item) => item.keyText === ""));

  const draft = classes.edit(items[0]?.name ?? "");
  assert.ok(draft !== null);
  assert.equal(resettableRow(classes, draft), null);

  const view = classes.rows[0];
  assert.ok(view !== undefined);
  const before = classes.value;
  const removed = removeSizeRow(classes, view);
  assert.ok(removed.text.length > 0);
  removed.undo();
  assert.deepEqual(classes.value, before);
});

test("rule rows, delete with Undo, and moves that stop at the ends", async () => {
  const { model } = await windowModel();
  const rules = as(model.session.setting("classifier.rules"), RuleListModel);
  const count = rules.value.length;
  assert.ok(count >= 2);

  const items = ruleRowItems(rules, model.session, [1]);
  assert.equal(items[0]?.canMoveUp, false);
  assert.equal(items[1]?.matches, true);
  assert.equal(items.at(-1)?.canMoveDown, false);
  assert.match(items[0]?.title ?? "", /^1\. /);

  assert.equal(moveRule(rules, 0, -1), null);
  assert.equal(moveRule(rules, 0, 1), 1);

  const before = rules.value.slice();
  const deleted = deleteRule(rules, count - 1);
  assert.equal(rules.value.length, count - 1);
  assert.equal(deleted.focus, count - 2);
  deleted.undo();
  assert.deepEqual(rules.value, before);
});

test("Test a command: empty, an unready draft, allotd's answer, a stopped service", async () => {
  const { model, client } = await windowModel();
  const rules = as(model.session.setting("classifier.rules"), RuleListModel);

  assert.deepEqual(await testCommand(model.session, "  ", savedRules(rules), english), {
    lines: [],
    test: null,
  });

  const draft = rules.add();
  assert.equal(draftRules(rules, draft), null);
  assert.deepEqual(await testCommand(model.session, "make", null, english), {
    lines: [DRAFT_NOT_READY_TEXT],
    test: null,
  });

  const answered = await testCommand(model.session, "cargo build", savedRules(rules), english);
  assert.deepEqual(answered, { lines: ["unavailable"], test: null });
  assert.equal(client.classified[0]?.shell, "bash");
  assert.equal(client.classified[0]?.line, "cargo build");

  client.reads.push(failure("unreachable"));
  await model.refresh();
  await settleAll();
  assert.deepEqual(await testCommand(model.session, "make", savedRules(rules), english), {
    lines: [SERVICE_STOPPED_TEXT],
    test: null,
  });
});

test("print-only flags stay a string list on their own section", async () => {
  const { model } = await windowModel();
  const commands = model.session.pages.find((page) => page.spec.id === "commands");
  assert.ok(commands !== undefined);
  const flags = pageSections(commands).find((section) => section.kind === "settings");

  assert.ok(flags?.kind === "settings" && flags.settings[0] instanceof StringListModel);
});

test("the window speaks a banner that appears on the page shown, and not on another page", async () => {
  const { model, client } = await windowModel();
  /** @type {string[]} */
  const spoken = [];
  model.onAnnounce = (text) => spoken.push(text);
  model.showPage("admission");

  client.reads.push(failure("unreachable"));
  await model.refresh();
  await settleAll();
  model.showPage("service");

  assert.deepEqual(spoken, [
    "The service isn't answering on port 47823. Trying again every 5 s.",
  ]);
});

test("a version that cannot be read is logged, not swallowed", async () => {
  const { model, client, logged } = await windowModel({ load: false });
  client.healthVersion = async () => {
    throw new Error("the bus dropped");
  };

  await model.refresh();
  await settleAll();

  assert.deepEqual(logged, ["Allot: cannot read the service version: Error: the bus dropped"]);
});
