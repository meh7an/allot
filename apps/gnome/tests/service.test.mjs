import assert from "node:assert/strict";
import { test } from "node:test";

import { serviceBanner } from "../allot@mehran.tech/lib/service-banner.js";
import {
  SystemdServiceLauncher,
  systemdUnitExec,
  systemdUnitFile,
} from "../allot@mehran.tech/lib/service-launcher.js";
import { serviceMenuItem, stopConfirmation } from "../allot@mehran.tech/lib/service-menu.js";
import { scenario } from "./handoff.mjs";

/** @import { ServiceSystem } from "../allot@mehran.tech/lib/service-launcher.js" */

const time = (/** @type {number} */ ms) => `t${ms}`;
const HOME = "/home/user";
const UNIT_FILE = "/home/user/.config/systemd/user/allotd.service";
const EXE = "/home/user/.allot/bin/allotd";

/**
 * A fake system that records what it ran and started.
 *
 * @param {object} [options]
 * @param {string | null} [options.unit] The unit file's text.
 * @param {boolean} [options.executable]
 * @param {boolean} [options.systemctlOk]
 * @param {string} [options.spawnError]
 * @returns {ServiceSystem & { ran: string[][], spawned: string[] }}
 */
function fakeSystem(options = {}) {
  /** @type {string[][]} */
  const ran = [];
  /** @type {string[]} */
  const spawned = [];
  return {
    ran,
    spawned,
    readText: async (path) => (path === UNIT_FILE ? (options.unit ?? null) : null),
    isExecutable: async () => options.executable ?? true,
    run: async (argv) => {
      ran.push(argv);
      return options.systemctlOk ?? true;
    },
    spawn: (path) => {
      if (options.spawnError !== undefined) {
        throw new Error(options.spawnError);
      }
      spawned.push(path);
    },
  };
}

/**
 * @param {ServiceSystem} system
 * @returns {SystemdServiceLauncher}
 */
function launcher(system) {
  return new SystemdServiceLauncher(
    { home: HOME, configHome: null, root: `${HOME}/.allot` },
    system,
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
      "2 jobs are running and 1 request is waiting.\n\nHeavy commands run unchecked",
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

test("Start uses the unit when it starts this install's allotd", async () => {
  const system = fakeSystem({ unit: `ExecStart="${EXE}"\n` });

  assert.equal(await launcher(system).start(), null);

  assert.deepEqual(system.ran, [["systemctl", "--user", "start", "allotd.service"]]);
  assert.deepEqual(system.spawned, []);
});

test("Start never starts the unit of another install", async () => {
  const system = fakeSystem({ unit: "ExecStart=/opt/dev/allotd\n" });

  assert.equal(await launcher(system).start(), null);

  assert.deepEqual(system.ran, []);
  assert.deepEqual(system.spawned, [EXE]);
});

test("Start spawns allotd without a unit, and when systemctl fails", async () => {
  const noUnit = fakeSystem();
  assert.equal(await launcher(noUnit).start(), null);
  assert.deepEqual(noUnit.ran, []);
  assert.deepEqual(noUnit.spawned, [EXE]);

  const failing = fakeSystem({ unit: `ExecStart=${EXE}\n`, systemctlOk: false });
  assert.equal(await launcher(failing).start(), null);
  assert.equal(failing.ran.length, 1);
  assert.deepEqual(failing.spawned, [EXE]);
});

test("Start says when allotd is missing or cannot run", async () => {
  assert.equal(
    await launcher(fakeSystem({ executable: false })).start(),
    `${EXE} is missing; run allot install`,
  );
  assert.equal(await launcher(fakeSystem({ spawnError: "denied" })).start(), `${EXE}: denied`);
});
