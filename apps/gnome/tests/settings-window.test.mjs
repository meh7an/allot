import assert from "node:assert/strict";
import { test } from "node:test";

import { decodeFitPreview } from "../allot@mehran.tech/contract.g.mjs";
import { resetField } from "../allot@mehran.tech/lib/field-reset.js";
import { FitPreviews } from "../allot@mehran.tech/lib/fit-previews.js";
import {
  LOWERING_OFF_TEXT,
  SERVICE_STOPPED_TEXT,
  benchmarksText,
  estimatesText,
  fitsCaption,
  fitsText,
  headroomText,
  historyText,
} from "../allot@mehran.tech/lib/live-text.js";
import {
  changedPanelLines,
  meterToggles,
  notificationToggles,
  resetPanel,
  setToggle,
  toggleEnabled,
  toggleValue,
} from "../allot@mehran.tech/lib/panel-toggles.js";
import { restartService } from "../allot@mehran.tech/lib/service-launcher.js";
import { STOP_CONFIRM_TITLE, stopConfirmation } from "../allot@mehran.tech/lib/service-menu.js";
import {
  STOPPED_BY_YOU_TEXT,
  serviceButtonText,
  serviceRow,
  sessionServiceStatus,
} from "../allot@mehran.tech/lib/service-row.js";
import {
  DurationModel,
  NumberModel,
  SwitchModel,
} from "../allot@mehran.tech/lib/setting-values.js";
import {
  PANEL_PAGE,
  SERVICE_PAGE,
  bannerActionLabel,
  restartNote,
  settingsBanners,
} from "../allot@mehran.tech/lib/settings-banners.js";
import { SAVE_DELAY_MS } from "../allot@mehran.tech/lib/settings-session.js";
import { defaultSettings } from "../allot@mehran.tech/lib/settings.js";
import {
  fieldResetText,
  joined,
  localeNumbers,
  resetAllConfirmation,
  resetButtonName,
  resetPageConfirmation,
  saveStatusText,
} from "../allot@mehran.tech/lib/settings-text.js";
import {
  as,
  english,
  failure,
  fire,
  fixtureState,
  loaded,
  setValue,
  settleAll,
} from "./settings-support.mjs";
import { fixtureState as uiFixtureState, fixtureText } from "./support.mjs";

/** @import { Reach } from "../allot@mehran.tech/lib/service-row.js" */

/** @type {Reach} */
const PORT_REACH = { kind: "port", port: 47823 };
/** @type {Reach} */
const BUS_REACH = { kind: "bus" };

// The Settings window's shared texts and helpers.

/**
 * @param {number} full
 * @param {number} shrunk
 * @param {boolean} [more]
 */
const counts = (full, shrunk, more = false) => ({ full, shrunk, more });

test("the save status reads as the design says", () => {
  assert.equal(saveStatusText({ kind: "saved" }), "All changes saved");
  assert.equal(saveStatusText({ kind: "saving" }), "Saving…");
  assert.equal(saveStatusText({ kind: "unsaved", count: 1 }), "1 change not saved");
  assert.equal(saveStatusText({ kind: "unsaved", count: 3 }), "3 changes not saved");
});

test("numbers are grouped with at most two decimals", () => {
  const number = localeNumbers("en-US");

  assert.equal(number(5000), "5,000");
  assert.equal(number(3.3), "3.3");
  assert.equal(number(2.456), "2.46");
  assert.equal(number(15), "15");
});

test("joined lists read as a sentence", () => {
  assert.equal(joined(["a"]), "a");
  assert.equal(joined(["a", "b"]), "a and b");
  assert.equal(joined(["a", "b", "c"]), "a, b and c");
});

test("reset texts name the default, with title-case buttons", async () => {
  const { session } = await loaded();
  const port = as(session.setting("daemon.port"), NumberModel);
  const days = as(session.setting("retention.lease_days"), NumberModel);
  const idle = as(session.setting("leases.idle_release_secs"), DurationModel);

  assert.equal(fieldResetText(port, english), "Port reset to 47823.");
  assert.equal(resetButtonName(port, english), "Reset Port to 47823");
  assert.equal(fieldResetText(idle, english), "Idle for reset to 3 minutes.");

  const page = resetPageConfirmation("Leases", [days, port], english);
  assert.equal(page.title, "Reset 2 settings on Leases?");
  assert.equal(
    page.body,
    "These go back to their defaults:\nKeep ended leases for: 30 days\nPort: 47823",
  );
  assert.equal(page.button, "Reset 2 Settings");

  assert.equal(resetAllConfirmation(1).title, "Reset the 1 changed setting?");
  assert.equal(resetAllConfirmation(1).button, "Reset 1 Setting");
  assert.equal(resetAllConfirmation(12).title, "Reset all 12 changed settings?");
});

test("the Panel page shows no service banner", async () => {
  const { session } = await loaded();

  assert.deepEqual(settingsBanners(session, PANEL_PAGE, PORT_REACH, english), []);
});

test("stopped by you offers Start Service", async () => {
  const { session, client } = await loaded({ stopRecord: '{"by": "panel", "stopped_at_ms": 1}' });
  client.reads.push(failure("unreachable"));
  await session.refresh();

  const banners = settingsBanners(session, "admission", PORT_REACH, english);

  assert.equal(banners.length, 1);
  assert.equal(banners[0]?.message, "The service is stopped. Start it to change these settings.");
  assert.deepEqual(banners[0]?.actions, ["start_service"]);
  assert.ok(session.readOnly);
});

test("not answering names the port this panel dials", async () => {
  const { session, client } = await loaded();
  client.reads.push(failure("unreachable"));
  await session.refresh();

  const [banner] = settingsBanners(session, "leases", PORT_REACH, english);

  assert.equal(banner?.severity, "warning");
  assert.equal(
    banner?.message,
    "The service isn't answering on port 47823. Trying again every 5 s.",
  );
  assert.deepEqual(banner?.actions, ["try_now", "open_logs"]);
});

test("a refused key offers Restart Service", async () => {
  const { session, client } = await loaded();
  client.reads.push(failure("key"));
  await session.refresh();

  const [banner] = settingsBanners(session, SERVICE_PAGE, PORT_REACH, english);

  assert.equal(banner?.severity, "error");
  assert.equal(
    banner?.message,
    "The service refused this panel's key. Restart the service to make a new one.",
  );
  assert.deepEqual(banner?.actions, ["restart_service"]);
});

test("a new port waits for a restart on the Service page only", async () => {
  const { session } = await loaded();

  const service = settingsBanners(session, SERVICE_PAGE, PORT_REACH, english);

  assert.deepEqual(
    service.map((banner) => banner.message),
    ["Restart the service to use port 47999."],
  );
  assert.deepEqual(service[0]?.actions, ["restart_service"]);
  assert.deepEqual(settingsBanners(session, "admission", PORT_REACH, english), []);
  assert.equal(
    restartNote(session, "daemon.port"),
    "Restart needed. The service still listens on 47823.",
  );
  assert.equal(restartNote(session, "hook.fail_open"), null);
});

test("a rejected file is an error banner and keeps the pages read-only", async () => {
  const state = fixtureState((json) => {
    json.file_error = "expected a number at line 3";
  });
  const { session } = await loaded({ state });

  const [banner] = settingsBanners(session, "admission", PORT_REACH, english);

  assert.ok(session.readOnly);
  assert.equal(banner?.severity, "error");
  assert.equal(
    banner?.message,
    "config.toml has an error: expected a number at line 3. The service keeps using its last " +
      "good settings. Fix the file to change settings here.",
  );
  assert.deepEqual(banner?.actions, ["open_file", "check_again"]);
});

test("a hand-edit conflict names the change that was not saved", async () => {
  const { session, client, timers } = await loaded();
  client.writes.push(failure("conflict", { revision: "0000000000000001" }));
  as(session.setting("estimates.margin_pct"), NumberModel).commit(15);

  await fire(timers);

  const [banner] = settingsBanners(session, "admission", PORT_REACH, english);
  assert.equal(banner?.severity, "warning");
  assert.equal(
    banner?.message,
    "config.toml changed outside Settings. Showing the file's values. Your change to Safety " +
      "margin (15 %) was not saved.",
  );
  assert.deepEqual(banner?.actions, ["apply_mine", "keep_file_values"]);
});

test("a held file keeps the changes and offers Try Again", async () => {
  const held = "config.toml is open in another program that blocks changes.";
  const { session, client, timers } = await loaded();
  client.writes.push(failure("held", { message: held }));
  as(session.setting("admission.headroom_pct"), NumberModel).commit(20);

  await fire(timers);

  const [banner] = settingsBanners(session, "admission", PORT_REACH, english);
  assert.equal(banner?.message, `Your changes are not saved yet. ${held}`);
  assert.deepEqual(banner?.actions, ["save_again"]);
  assert.deepEqual(session.saveStatus, { kind: "unsaved", count: 1 });
});

test("banner buttons are title case", () => {
  assert.equal(bannerActionLabel("start_service"), "Start Service");
  assert.equal(bannerActionLabel("keep_file_values"), "Keep File's Values");
  assert.equal(bannerActionLabel("save_again"), "Try Again");
});

/** @param {boolean} gpu */
function kept(gpu) {
  return {
    cores: 2.4,
    total_cores: 16,
    ram_mb: 4915.2,
    total_ram_mb: 32768,
    gpu_pct: gpu ? 15 : null,
    vram_mb: gpu ? 1228.8 : null,
    total_vram_mb: gpu ? 8192 : null,
  };
}

test("the headroom sentence names what is kept free", () => {
  assert.equal(
    headroomText(kept(true), english),
    "Keeps 2.4 of 16 cores, 4.8 of 32 GB memory, 15 % of the GPU and 1.2 of 8 GB video memory " +
      "free.",
  );
  assert.equal(
    headroomText(kept(false), english),
    "Keeps 2.4 of 16 cores and 4.8 of 32 GB memory free.",
  );
});

test("the estimates sentence follows the margin and the runs", () => {
  assert.equal(
    estimatesText({ lower: true, minRuns: 3, marginPct: 10 }, english),
    "A command that asks for 8 cores but used about 3 in its last 3 runs is reserved 3.3 " +
      "cores. Requests never go below 0.25 cores, 256 MB, or their minimum cores, and run time " +
      "never lowers.",
  );
  assert.ok(
    estimatesText({ lower: true, minRuns: 1, marginPct: 0 }, english).includes(
      "in its last run is reserved 3 cores.",
    ),
  );
  assert.equal(
    estimatesText({ lower: false, minRuns: 3, marginPct: 10 }, english),
    LOWERING_OFF_TEXT,
  );
});

test("fits read at full size and shrunk", () => {
  assert.equal(fitsText(counts(0, 0), 4, english), "None fit");
  assert.equal(fitsText(counts(0, 1), 4, english), "1 at 4 cores");
  assert.equal(fitsText(counts(1, 2), 4, english), "1 at full size, 2 more at 4 cores");
  assert.equal(fitsText(counts(1, 0), 1, english), "1 at full size");
  assert.equal(fitsText(counts(99, 0, true), 1, english), "at least 99 at full size");
  assert.equal(
    fitsText(counts(2, 99, true), 0.5, english),
    "2 at full size, at least 99 more at 0.5 cores",
  );
});

test("the fixture preview fills every class", () => {
  const preview = decodeFitPreview(JSON.parse(fixtureText("fit-preview.json")));
  const cpu = preview.classes.find((fit) => fit.name === "cpu_heavy");
  assert.ok(cpu !== undefined);

  assert.equal(fitsText(cpu.now, cpu.min_cores, english), "1 at 4 cores");
  assert.equal(fitsText(cpu.idle, cpu.min_cores, english), "1 at full size, 1 more at 4 cores");
  assert.ok(fitsCaption(37.6, english).startsWith("Based on this machine now (38 %"));
});

test("the benchmarks sentence joins equal limits", () => {
  assert.equal(
    benchmarksText({ cpuPct: 10, gpuPct: 10, quietSecs: 10, maxWaitSecs: 60 }, english),
    "A benchmark starts once CPU and GPU stay at or below 10 % for 10 seconds. If that takes " +
      "longer than 1 minute, it starts anyway, marked not quiet.",
  );
  assert.equal(
    benchmarksText({ cpuPct: 10, gpuPct: 20, quietSecs: 90, maxWaitSecs: 600 }, english),
    "A benchmark starts once CPU stays at or below 10 % and GPU at or below 20 % for 90 " +
      "seconds. If that takes longer than 10 minutes, it starts anyway, marked not quiet.",
  );
});

test("the history sentence groups large counts", () => {
  assert.equal(
    historyText({ leaseDays: 30, historyDays: 90, keepRuns: 20, maxRuns: 5000 }, english),
    "Keeps ended leases for 30 days and job runs for 90 days. Each command keeps at least its " +
      "newest 20 runs, and never more than 5,000 runs are stored.",
  );
});

test("the service row follows the status and the stop record", () => {
  const running = serviceRow("running", "ready", "0.5.0", PORT_REACH);
  assert.equal(running.text, "Running · version 0.5.0 · port 47823");
  assert.equal(serviceButtonText(running.button), "Stop Service…");
  assert.equal(running.opensLogs, false);

  const stopped = serviceRow("stopped", "stopped", null, PORT_REACH);
  assert.equal(stopped.text, STOPPED_BY_YOU_TEXT);
  assert.equal(stopped.button, "start");
  assert.equal(stopped.opensLogs, false);

  const down = serviceRow("stopped", "down", null, PORT_REACH);
  assert.equal(down.text, "Not answering on port 47823.");
  assert.equal(serviceButtonText(down.button), "Start Service");
  assert.equal(down.opensLogs, true);

  const starting = serviceRow("starting", "down", null, PORT_REACH);
  assert.equal(starting.button, "starting");
  assert.equal(serviceButtonText(starting.button), "Starting…");
});

test("a panel on the session bus names the bus, never a port", async () => {
  const running = serviceRow("running", "ready", "0.5.0", BUS_REACH);
  assert.equal(running.text, "Running · version 0.5.0");
  assert.equal(serviceRow("running", "ready", null, BUS_REACH).text, "Running");
  const down = serviceRow("stopped", "down", null, BUS_REACH);
  assert.equal(down.text, "Not on the session bus.");
  assert.equal(down.opensLogs, true);

  const { session, client } = await loaded();
  client.reads.push(failure("unreachable"));
  await session.refresh();
  const [banner] = settingsBanners(session, "leases", BUS_REACH, english);
  assert.equal(
    banner?.message,
    "The service isn't on the session bus. Trying again every 5 s.",
  );
});

test("the preferences window reads the service status from its session", () => {
  assert.equal(sessionServiceStatus("loading", false), null);
  assert.equal(sessionServiceStatus("ready", false), "running");
  assert.equal(sessionServiceStatus("stopped", false), "stopped");
  assert.equal(sessionServiceStatus("down", false), "stopped");
  assert.equal(sessionServiceStatus("key", false), "incompatible");
  assert.equal(sessionServiceStatus("down", true), "starting");
});

test("the stop warning says the stop lasts", () => {
  const text = stopConfirmation(uiFixtureState("busy"));

  assert.equal(STOP_CONFIRM_TITLE, "Stop the service?");
  assert.ok(
    text?.endsWith(
      "Jobs keep running, and new heavy commands run without leases until you start the " +
        "service again.",
    ),
  );
});

test("reset then Undo writes the old number back", async () => {
  const state = fixtureState((json) => setValue(json, "leases.heartbeat_ttl_secs", 120));
  const { session } = await loaded({ state });
  const model = as(session.setting("leases.heartbeat_ttl_secs"), DurationModel);

  const undo = resetField(model);

  assert.equal(model.value, 60);
  assert.ok(undo !== null);
  undo();
  assert.equal(model.value, 120);
  assert.equal(model.issue, null);
});

test("reset then Undo turns an off duration off again", async () => {
  const state = fixtureState((json) => setValue(json, "leases.idle_release_secs", 0));
  const { session } = await loaded({ state });
  const model = as(session.setting("leases.idle_release_secs"), DurationModel);

  const undo = resetField(model);

  assert.ok(model.isOn);
  undo?.();
  assert.equal(model.isOn, false);
  assert.equal(model.value, 0);
});

test("reset then Undo turns a switch back", async () => {
  const state = fixtureState((json) => setValue(json, "hook.fail_open", false));
  const { session } = await loaded({ state });
  const model = as(session.setting("hook.fail_open"), SwitchModel);

  const undo = resetField(model);

  assert.equal(model.value, true);
  undo?.();
  assert.equal(model.value, false);
});

test("active previews ask with the unsaved edits", async () => {
  const { session, client, timers } = await loaded();
  const fits = new FitPreviews(session, timers);

  fits.setActive(true);
  await settleAll();

  assert.equal(client.previews.length, 1);
  assert.equal(fits.unavailable, null);
});

test("inactive previews ask nothing", async () => {
  const { session, client, timers } = await loaded();
  const fits = new FitPreviews(session, timers);

  fits.sessionChanged();
  await settleAll();

  assert.deepEqual(client.previews, []);
  assert.deepEqual(timers.delays, []);
});

test("a change asks again after the save delay", async () => {
  const { session, timers } = await loaded();
  const fits = new FitPreviews(session, timers);
  fits.setActive(true);
  await settleAll();

  fits.sessionChanged();

  assert.deepEqual(timers.delays, [SAVE_DELAY_MS]);
});

test("a stopped service explains why no preview shows", async () => {
  const { session, client, timers } = await loaded({
    stopRecord: '{"by": "cli", "stopped_at_ms": 1}',
  });
  client.reads.push(failure("unreachable"));
  await session.refresh();
  const fits = new FitPreviews(session, timers);

  assert.equal(fits.unavailable, SERVICE_STOPPED_TEXT);
  assert.equal(fits.fit("classes", "cpu_heavy"), null);
});

test("panel toggles write their own setting", () => {
  const settings = defaultSettings();
  for (const item of [...meterToggles(false), ...notificationToggles()]) {
    const before = JSON.stringify(settings);
    const was = toggleValue(settings, item);
    setToggle(settings, item, !was);

    assert.notEqual(JSON.stringify(settings), before, item.label);
    setToggle(settings, item, was);
  }

  assert.deepEqual(settings, defaultSettings());
});

test("Drive activity needs Drives", () => {
  const activity = meterToggles(false).find((item) => item.label === "Drive activity");
  assert.ok(activity !== undefined);
  const settings = defaultSettings();

  assert.ok(toggleEnabled(settings, activity));
  settings.meters.drives = false;
  assert.equal(toggleEnabled(settings, activity), false);
});

test("VRAM reads GPU memory where the GPU shares memory", () => {
  assert.ok(meterToggles(true).some((item) => item.label === "GPU memory"));
  assert.ok(meterToggles(false).some((item) => item.label === "VRAM"));
  assert.ok(notificationToggles().some((item) => item.label === "Killed outside the popup"));
});

test("the Panel page's Reset Page resets only its own settings", () => {
  const settings = defaultSettings();
  settings.meters.cpu = false;
  settings.notifications.job_finished = true;
  settings.resources_expanded = true;
  settings.first_run_done = true;

  assert.deepEqual(changedPanelLines(settings, false), ["CPU: On", "Job finished: Off"]);
  resetPanel(settings);

  assert.deepEqual(changedPanelLines(settings, false), []);
  assert.deepEqual(settings.meters, defaultSettings().meters);
  assert.deepEqual(settings.notifications, defaultSettings().notifications);
  assert.ok(settings.resources_expanded);
  assert.ok(settings.first_run_done);
});

/** A restart's client and launcher, with every call recorded. */
function restartFakes(/** @type {{ unit: boolean, healthy?: number }} */ options) {
  /** @type {string[]} */
  const calls = [];
  let healthy = options.healthy ?? 0;
  const client = {
    shutdown: async (/** @type {{ record: boolean }} */ shutdown) => {
      calls.push(`shutdown record=${shutdown.record}`);
      return null;
    },
    healthVersion: async () => {
      calls.push("health");
      healthy -= 1;
      return healthy >= 0 ? "0.5.0" : null;
    },
  };
  const launcher = {
    restartThroughUnit: async () => {
      calls.push("systemctl restart");
      return options.unit;
    },
    start: async () => {
      calls.push("start");
      return null;
    },
  };
  const timers = {
    setTimeout: (/** @type {() => void} */ callback) => {
      setImmediate(callback);
      return 0;
    },
    clearTimeout: () => {},
  };
  return { calls, client, launcher, timers };
}

test("restart goes through the systemd unit when it runs this allotd", async () => {
  const { calls, client, launcher, timers } = restartFakes({ unit: true });

  assert.equal(await restartService(client, launcher, timers), null);

  assert.deepEqual(calls, ["systemctl restart"]);
});

test("restart without the unit stops with no stop record, waits, then starts", async () => {
  const { calls, client, launcher, timers } = restartFakes({ unit: false, healthy: 2 });

  assert.equal(await restartService(client, launcher, timers), null);

  assert.deepEqual(calls, [
    "systemctl restart",
    "shutdown record=false",
    "health",
    "health",
    "health",
    "start",
  ]);
});
