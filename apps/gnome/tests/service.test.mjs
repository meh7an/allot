import assert from "node:assert/strict";
import { test } from "node:test";

import { bannerButtonText, serviceBanner } from "../allot@mehran.tech/lib/service-banner.js";
import {
  SystemdServiceLauncher,
  systemdUnitExec,
  systemdUnitFile,
} from "../allot@mehran.tech/lib/service-launcher.js";
import { serviceMenuItem, stopConfirmation } from "../allot@mehran.tech/lib/service-menu.js";
import {
  serviceButtonText,
  serviceRow,
  sessionServiceStatus,
} from "../allot@mehran.tech/lib/service-row.js";
import { scenario } from "./handoff.mjs";

/** @import { ServiceSystem } from "../allot@mehran.tech/lib/service-launcher.js" */

const time = (/** @type {number} */ ms) => `t${ms}`;
const HOME = "/home/user";
const UNIT_FILE = "/home/user/.config/systemd/user/allotd.service";
const EXE = "/home/user/.allot/bin/allotd";

/**
 * A fake system that records what it ran.
 *
 * @param {object} [options]
 * @param {string | null} [options.unit] The unit file's text.
 * @param {boolean} [options.systemctlOk]
 * @param {string} [options.output] What systemctl prints.
 * @returns {ServiceSystem & { ran: string[][] }}
 */
function fakeSystem(options = {}) {
  /** @type {string[][]} */
  const ran = [];
  return {
    ran,
    readText: async (path) => (path === UNIT_FILE ? (options.unit ?? null) : null),
    run: async (argv) => {
      ran.push(argv);
      return { ok: options.systemctlOk ?? true, output: options.output ?? "" };
    },
  };
}

/**
 * @param {ServiceSystem} system
 * @param {(() => Promise<boolean>) | null} [aliveOffBus]
 * @returns {SystemdServiceLauncher}
 */
function launcher(system, aliveOffBus = null) {
  return new SystemdServiceLauncher(
    { home: HOME, configHome: null, root: `${HOME}/.allot` },
    system,
    aliveOffBus,
  );
}

test("the banner follows the status, and a running service has none", () => {
  const { state } = scenario("one");

  assert.equal(serviceBanner("running", state, null, time), null);
  assert.deepEqual(serviceBanner("stopped", state, null, time), {
    text: "Service stopped · commands run unchecked",
    problem: "",
    button: "start",
    caption: `Last known · t${state.generated_at_ms}`,
  });
  assert.equal(serviceBanner("starting", state, null, time)?.button, "starting");
  assert.deepEqual(serviceBanner("incompatible", null, null, time), {
    text: "Service needs an update · run allot install",
    problem: "",
    button: "none",
    caption: "",
  });
});

test("the banner says why the last start failed", () => {
  assert.equal(serviceBanner("stopped", null, "no such file", time)?.problem, "no such file");
  assert.equal(serviceBanner("starting", null, "no such file", time)?.problem, "");
});

test("the service item follows the status", () => {
  assert.deepEqual(serviceMenuItem("running"), { title: "Stop Service…", action: "stop" });
  assert.deepEqual(serviceMenuItem("incompatible"), { title: "Stop Service…", action: "stop" });
  assert.deepEqual(serviceMenuItem("stopped"), { title: "Start Service", action: "start" });
  assert.deepEqual(serviceMenuItem("starting"), { title: "Starting Service…", action: "none" });
});

test("stopping asks only while jobs run or wait", () => {
  assert.equal(stopConfirmation(null), null);
  assert.equal(stopConfirmation(scenario("empty").state), null);
  assert.ok(stopConfirmation(scenario("one").state)?.startsWith("1 job is running.\n\n"));
  assert.ok(
    stopConfirmation(scenario("several").state)?.startsWith(
      "2 jobs are running and 1 request is waiting.\n\nJobs keep running",
    ),
  );
  assert.ok(stopConfirmation(scenario("c-queued").state)?.startsWith("2 requests are waiting."));
});

test("the unit file honours an absolute XDG_CONFIG_HOME only", () => {
  assert.equal(systemdUnitFile(HOME, null), UNIT_FILE);
  assert.equal(systemdUnitFile(HOME, "relative"), UNIT_FILE);
  assert.equal(systemdUnitFile(HOME, "/cfg"), "/cfg/systemd/user/allotd.service");
});

test("ExecStart reads plain and quoted, as allot install writes it", () => {
  assert.equal(systemdUnitExec(`[Service]\nExecStart=${EXE} --flag\n`), EXE);
  assert.equal(systemdUnitExec(`[Service]\r\nExecStart="${EXE}"\r\n`), EXE);
  assert.equal(systemdUnitExec('ExecStart="/a b/100%%/$$x\\"y"'), '/a b/100%/$x"y');
  assert.equal(systemdUnitExec('ExecStart="/never closed'), null);
  assert.equal(systemdUnitExec("[Service]\n"), null);
});

const START = ["systemctl", "--user", "start", "allotd.service"];
const RESTART = ["systemctl", "--user", "restart", "allotd.service"];
const NO_UNIT = "allotd.service is not installed; run allot install";

test("Start runs only systemctl start, whatever the unit file says", async () => {
  for (const unit of [`ExecStart="${EXE}"\n`, "ExecStart=/opt/dev/allotd\n", null]) {
    const system = fakeSystem({ unit });

    assert.equal(await launcher(system).start(), null);

    assert.deepEqual(system.ran, [START]);
  }
});

test("Start ends at once, naming allot install, when the unit does not exist", async () => {
  for (const output of [
    "Failed to start allotd.service: Unit allotd.service not found.\n",
    "Failed to start allotd.service: Unit allotd.service not loaded.\n",
    "Failed to start allotd.service: Unit allotd.service is masked.\n",
  ]) {
    const system = fakeSystem({ systemctlOk: false, output });

    assert.equal(await launcher(system).start(), NO_UNIT);
    assert.deepEqual(system.ran, [START]);
  }
});

test("Start reports another systemctl failure by its last line", async () => {
  const output = "Job failed.\nFailed to connect to user scope bus\n\n";
  assert.equal(
    await launcher(fakeSystem({ systemctlOk: false, output })).start(),
    "Failed to connect to user scope bus",
  );
  assert.equal(
    await launcher(fakeSystem({ systemctlOk: false })).start(),
    "systemctl --user start allotd.service failed",
  );
});

test("Start leaves an allotd that runs off the bus alone", async () => {
  const system = fakeSystem();

  assert.equal(
    await launcher(system, async () => true).start(),
    "allotd already runs without the session bus; restart it outside the sandbox",
  );
  assert.deepEqual(system.ran, []);

  assert.equal(await launcher(system, async () => false).start(), null);
  assert.deepEqual(system.ran, [START]);
});

test("restartUnit runs systemctl restart and says when the unit is missing", async () => {
  const system = fakeSystem();
  assert.equal(await launcher(system).restartUnit(), null);
  assert.deepEqual(system.ran, [RESTART]);

  const missing = fakeSystem({ systemctlOk: false, output: "Unit allotd.service not found." });
  assert.equal(await launcher(missing).restartUnit(), NO_UNIT);
});

test("restartThroughUnit restarts only the unit of this install", async () => {
  const mine = fakeSystem({ unit: `ExecStart="${EXE}"\n` });
  assert.equal(await launcher(mine).restartThroughUnit(), true);
  assert.deepEqual(mine.ran, [RESTART]);

  const other = fakeSystem({ unit: "ExecStart=/opt/dev/allotd\n" });
  assert.equal(await launcher(other).restartThroughUnit(), false);
  assert.deepEqual(other.ran, []);

  const failing = fakeSystem({ unit: `ExecStart=${EXE}\n`, systemctlOk: false });
  assert.equal(await launcher(failing).restartThroughUnit(), false);
});

test("the off-bus state has its own banner, menu item and row", () => {
  assert.deepEqual(serviceBanner("off_bus", null, null, time), {
    text: "Service runs without the session bus · restart it outside the sandbox",
    problem: "",
    button: "restart",
    caption: "",
  });
  assert.equal(serviceBanner("off_bus", null, "unit missing", time)?.problem, "unit missing");
  assert.equal(bannerButtonText("restart"), "Restart Service");
  assert.equal(bannerButtonText("starting"), "Starting…");
  assert.equal(bannerButtonText("start"), "Start Service");
  assert.deepEqual(serviceMenuItem("off_bus"), { title: "Restart Service", action: "restart" });

  const row = serviceRow("off_bus", "off_bus", null, { kind: "bus" });
  assert.equal(row.button, "restart");
  assert.equal(serviceButtonText(row.button), "Restart Service");
  assert.match(row.text, /without the session bus.*outside the sandbox/);
  assert.equal(sessionServiceStatus("off_bus", false), "off_bus");
  assert.equal(sessionServiceStatus("off_bus", true), "starting");
});
