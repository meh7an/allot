// The KDE Settings window's logic: what its views show, and the widget's side of it.

import assert from "node:assert/strict";
import { test } from "node:test";

import { defaultSettings } from "../../gnome/allot@mehran.tech/lib/settings.js";
import {
  DurationModel,
  NumberModel,
  RuleListModel,
  StringListModel,
  SwitchModel,
} from "../../gnome/allot@mehran.tech/lib/setting-values.js";
import { as, english, loaded } from "../../gnome/tests/settings-support.mjs";
import { FakeTimers, fixtureText, settle } from "../../gnome/tests/support.mjs";
import {
  bannerViews,
  chipsView,
  escaped,
  helpText,
  localeNumber,
  numberView,
  offSwitchView,
  panelResetConfirmation,
  panelView,
  resetSetting,
  rowKind,
  switchView,
  testAgainst,
} from "../src/settings-view.js";
import { fakeHost } from "./support.mjs";

/** @import { WindowHost } from "../../gnome/allot@mehran.tech/lib/settings-window.js" */

const HOME = "/home/user one/.allot";

/** Formats as QML's toLocaleString does in an en-US locale, with the decimals asked for. */
const qtEnglish = (/** @type {number} */ value, /** @type {number} */ decimals) =>
  new Intl.NumberFormat("en-US", {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(value);

/** @returns {WindowHost} */
function windowHost() {
  return { confirm: () => {}, notify: () => {}, openPath: () => {}, now: () => 0 };
}

test("numbers show at most two decimals, and no trailing zeros", () => {
  assert.equal(localeNumber(12000, qtEnglish), "12,000");
  assert.equal(localeNumber(2.5, qtEnglish), "2.5");
  assert.equal(localeNumber(0.125, qtEnglish), "0.13");
  assert.equal(localeNumber(3.999, qtEnglish), "4");
});

test("each setting gets the row of its kind", async () => {
  const { session } = await loaded();
  const kinds = new Map(
    session.pages.flatMap((page) =>
      page.groups.flatMap((group) => group.settings.map((model) => [model.key, rowKind(model)])),
    ),
  );

  assert.equal(kinds.get("estimates.lower"), "switch");
  assert.equal(kinds.get("admission.headroom_pct"), "number");
  assert.equal(kinds.get("leases.idle_release_secs"), "duration_off");
  assert.equal(kinds.get("classifier.light_flags"), "chips");
  assert.equal(kinds.get("classes"), "none");
  assert.equal(kinds.get("classifier.rules"), "none");
});

test("a number row: the port is never grouped, and a duration offers its units", async () => {
  const { session } = await loaded();
  const portModel = as(session.setting("daemon.port"), NumberModel);
  const port = numberView(portModel, english, false, null);
  const quiet = as(session.setting("benchmark.quiet_secs"), DurationModel);
  const idle = as(session.setting("leases.idle_release_secs"), DurationModel);

  assert.equal(port.grouped, false);
  assert.equal(port.integer, true);
  assert.equal(port.resetVisible, true, "the fixture's port differs from the default");
  assert.equal(numberView(portModel, english, false, null, false).resetVisible, false);
  assert.equal(numberView(quiet, english, false, null).units.join(","), "seconds,minutes,hours");
  assert.equal(numberView(idle, english, false, null).indented, true);
  assert.equal(numberView(quiet, english, true, null).enabled, false);
});

test("a row's issue is its own line, apart from its notes", async () => {
  const { session } = await loaded();
  const quiet = as(session.setting("benchmark.quiet_secs"), DurationModel);

  quiet.commit(-1);
  const view = numberView(quiet, english, false, "Restart needed.");

  assert.ok(view.issue !== null && view.issue.length > 0);
  assert.deepEqual(view.notes, ["Restart needed."]);
});

test("switch, off switch and chips rows", async () => {
  const { session } = await loaded();
  const lower = switchView(as(session.setting("estimates.lower"), SwitchModel), english, false);
  const idle = offSwitchView(as(session.setting("leases.idle_release_secs"), DurationModel), true);
  const flags = as(session.setting("classifier.light_flags"), StringListModel);
  const chips = chipsView(flags, english, false);

  assert.equal(typeof lower.checked, "boolean");
  assert.equal(lower.resetVisible, false);
  assert.equal(idle.enabled, false);
  assert.match(idle.help, /<tt>leases\.idle_release_secs<\/tt>$/);
  assert.equal(chips.chips.length, flags.value.length);
  assert.equal(chips.chips[0]?.name, `Remove ${flags.value[0]}`);
  assert.equal(chips.addLabel, `Add to ${flags.label}`);
});

test("a field's reset gives its notice and Undo", async () => {
  const { session } = await loaded();
  const port = as(session.setting("daemon.port"), NumberModel);

  const reset = resetSetting(port, english);

  assert.match(reset.text, /reset to 47823\.$/);
  assert.ok(reset.undo !== null);
});

test("help reads as rich text with its key in monospace, and markup is escaped", () => {
  assert.equal(helpText("a < b & c", "x.y"), "a &lt; b &amp; c&nbsp;&nbsp;<tt>x.y</tt>");
  assert.equal(helpText("Only help.", null), "Only help.");
  assert.equal(escaped('"<b>"'), "&quot;&lt;b&gt;&quot;");
});

test("banners carry their buttons' titles", () => {
  const views = bannerViews([
    { severity: "warning", message: "Not answering.", actions: ["try_now", "open_logs"] },
  ]);

  assert.deepEqual(views, [
    {
      severity: "warning",
      text: "Not answering.",
      actions: [
        { id: "try_now", label: "Try Now" },
        { id: "open_logs", label: "Open Logs" },
      ],
    },
  ]);
});

test("the Panel page: Drive activity needs Drives; Reset asks only when changed", () => {
  const settings = defaultSettings();

  let view = panelView(settings, true);
  assert.equal(view.meters[3]?.label, "GPU memory");
  assert.equal(view.meters[5]?.indented, true);
  assert.equal(view.canReset, false);
  assert.equal(panelResetConfirmation(settings, true), null);

  settings.meters.drives = false;
  view = panelView(settings, false);
  assert.equal(view.meters[5]?.enabled, false);
  assert.equal(view.canReset, true);
  assert.equal(panelResetConfirmation(settings, false)?.button, "Reset 1 Setting");
});

test("Test a command has nothing to test before a rule dialog first opens", async () => {
  const { session } = await loaded();
  const rules = as(session.setting("classifier.rules"), RuleListModel);

  assert.equal(testAgainst(null, null), null);
  assert.deepEqual(testAgainst(rules, null), { changes: [], rules: rules.value });
  assert.equal(testAgainst(rules, rules.add()), null);
});

/** A started widget against allotd answering the busy state and the settings-state fixture. */
async function startedWidget() {
  const timers = new FakeTimers();
  /** @type {string[]} */
  const posts = [];
  const subject = fakeHost({
    timers,
    files: { [`${HOME}/token`]: "t", [`${HOME}/tray.json`]: '{"first_run_shown":true}' },
    daemon: (request) => {
      if (request.method === "POST") {
        posts.push(request.url);
        return { status: 202, body: "" };
      }
      return request.url.endsWith("/settings")
        ? { status: 200, body: fixtureText("settings-state.json") }
        : { status: 200, body: fixtureText("ui-state-idle.json") };
    },
  });
  return { subject, posts };
}

test("the Settings window waits for the widget's client, then keeps one model", async () => {
  const { subject } = await startedWidget();

  const early = subject.widget.settingsWindow(windowHost(), english);
  subject.widget.start();
  const model = await early;
  await settle();

  assert.deepEqual(model.reach, { kind: "port", port: 47823 });
  assert.equal(await subject.widget.settingsWindow(windowHost(), english), model);
  await model.refresh();
  assert.equal(model.session.status, "ready");
  subject.widget.stop();
});

test("Stop Service from the window goes through the popup's poller", async () => {
  const { subject, posts } = await startedWidget();
  subject.widget.start();
  await settle();
  const model = await subject.widget.settingsWindow(windowHost(), english);
  await model.refresh();
  await settle();

  model.pressService();
  for (let round = 0; round < 5; round += 1) {
    await settle();
  }

  assert.deepEqual(posts, ["http://127.0.0.1:47823/shutdown"]);
  subject.widget.stop();
});

test("the Panel page's switches and reset change the widget's settings and save", async () => {
  const { subject } = await startedWidget();
  subject.widget.start();
  await settle();
  const { meterToggles } = await import("../../gnome/allot@mehran.tech/lib/panel-toggles.js");
  const drives = meterToggles(false).find((toggle) => toggle.key === "drives");
  assert.ok(drives !== undefined);

  subject.widget.setPanelToggle(drives, false);
  assert.equal(subject.widget.model.settings.meters.drives, false);
  subject.widget.resetPanel();
  await settle();

  assert.equal(subject.widget.model.settings.meters.drives, true);
  const saves = subject.commands.filter((command) => command.includes("tray.json.tmp"));
  assert.equal(saves.length, 2);
  subject.widget.stop();
});
