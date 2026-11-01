import assert from "node:assert/strict";
import { test } from "node:test";

import { FakeTimers, fixtureText, settle } from "../../gnome/tests/support.mjs";
import { fakeHost } from "./support.mjs";

const HOME = "/home/user one/.allot";
const TOKEN_FILE = `${HOME}/token`;
const SETTINGS_FILE = `${HOME}/tray.json`;
const CONFIG_HOME = "/home/user one/.config";
const DAEMON_EXE = `${HOME}/bin/allotd`;
const UNIT_FILE = `${CONFIG_HOME}/systemd/user/allotd.service`;
const FIRST_RUN_SEEN = JSON.stringify({ first_run_shown: true });
const SHUTDOWN_URL = "http://127.0.0.1:47823/shutdown";

/**
 * A closed widget against a daemon that can go away and come back.
 *
 * @param {object} [options]
 * @param {string} [options.fixture]
 * @param {Record<string, string>} [options.files]
 * @param {string} [options.inhibited]
 * @param {Record<string, { "exit code": number, stdout: string }>} [options.answers]
 */
async function service(options = {}) {
  const timers = new FakeTimers();
  const body = fixtureText(`ui-state-${options.fixture ?? "busy"}.json`);
  const daemon = { up: true };
  /** @type {string[]} */
  const posts = [];
  const files = { [TOKEN_FILE]: "t", [SETTINGS_FILE]: FIRST_RUN_SEEN, ...options.files };
  const subject = fakeHost({
    timers,
    files,
    ...(options.inhibited === undefined ? {} : { inhibited: options.inhibited }),
    ...(options.answers === undefined ? {} : { answers: options.answers }),
    daemon: (request) => {
      if (!daemon.up) {
        return null;
      }
      if (request.method === "POST") {
        posts.push(request.url);
        daemon.up = false;
        return { status: 202, body: "" };
      }
      return { status: 200, body };
    },
  });
  subject.widget.start();
  await settle();
  // Added to the host, whose counts are getters a spread would freeze.
  return Object.assign(subject, {
    daemon,
    posts,
    files,
    poll: async () => {
      timers.fireAll();
      await settle();
    },
    /** @returns {string[]} The shell commands that wrote the settings file. */
    saves: () => subject.commands.filter((command) => command.includes("tray.json.tmp")),
  });
}

/** The daemon goes away and stays away for the two polls a stop takes. */
async function stopFromOutside(/** @type {Awaited<ReturnType<typeof service>>} */ subject) {
  subject.daemon.up = false;
  await subject.poll();
  await subject.poll();
}

test("starting copies the notifyrc to where Plasma looks for it", async () => {
  const subject = await service();

  assert.ok(
    subject.commands.some(
      (command) =>
        command.startsWith("cmp -s -- '/package/contents/notifications/allot.notifyrc'") &&
        command.includes("/home/user one/.local/share/knotifications6/allot.notifyrc"),
    ),
  );
  assert.deepEqual(subject.lines, ["Allot: service running: 0.2.0"]);
});

test("a notifyrc that cannot be copied is logged, and polling goes on", async () => {
  const subject = fakeHost({
    timers: new FakeTimers(),
    files: { [TOKEN_FILE]: "t" },
    daemon: () => ({ status: 200, body: fixtureText("ui-state-busy.json") }),
    copyFails: true,
  });

  subject.widget.start();
  await settle();

  assert.equal(subject.lines[0], "Allot: cannot install the notifyrc; notifications will not show");
  assert.equal(subject.requests.length, 1);
});

test("the saved settings are read at start", async () => {
  const subject = await service({
    files: {
      [SETTINGS_FILE]: JSON.stringify({
        meters: { gpu: false },
        notifications: { job_finished: true },
        resources_expanded: true,
        first_run_shown: true,
      }),
    },
  });

  assert.equal(subject.widget.meterCount, "4 of 5");
  assert.equal(subject.widget.notificationCount, "4 of 5");
  assert.equal(subject.widget.model.resources.isExpanded, true);
  assert.deepEqual(subject.saves(), [], "reading them does not save them again");
});

test("a toggle changes the card and the count, and saves in the other apps' format", async () => {
  const subject = await service();

  subject.widget.toggleMeter("cpu");
  await settle();

  assert.equal(subject.widget.meterCount, "4 of 5");
  assert.ok(!subject.widget.model.resources.meters.some((row) => row.id === "cpu"));
  const [save] = subject.saves();
  assert.ok(save?.startsWith(`mkdir -p -- '${HOME}' && printf %s '{\n  "meters"`));
  assert.ok(save?.includes('"cpu": false'));
  assert.ok(save?.endsWith(`mv -f -- '${SETTINGS_FILE}.tmp' '${SETTINGS_FILE}'`));
});

test("saves run one at a time, in order", async () => {
  const subject = await service();

  subject.widget.toggleNotification("job_finished");
  subject.widget.toggleNotification("job_finished");
  subject.widget.toggleResources();
  await settle();

  const saves = subject.saves();
  assert.equal(saves.length, 3);
  assert.ok(saves[0]?.includes('"job_finished": true'));
  assert.ok(saves[1]?.includes('"job_finished": false'));
  assert.ok(saves[2]?.includes('"resources_expanded": true'));
});

test("a settings file that cannot be written is logged", async () => {
  const subject = fakeHost({
    timers: new FakeTimers(),
    files: { [TOKEN_FILE]: "t" },
    daemon: () => ({ status: 200, body: fixtureText("ui-state-busy.json") }),
    writeFails: true,
  });
  subject.widget.start();
  await settle();

  subject.widget.toggleResources();
  await settle();

  assert.ok(subject.lines.includes(`Allot: cannot save ${SETTINGS_FILE}`));
});

test("while the service is down the banner offers Start Service and no card shows", async () => {
  const subject = await service();
  subject.daemon.up = false;
  await subject.poll();

  assert.equal(subject.widget.model.banner?.button, "start");
  assert.ok(subject.widget.model.banner?.caption.startsWith("Last known · t"));
  assert.equal(subject.widget.model.resources.isVisible, false);
  assert.deepEqual(subject.widget.serviceItem, { title: "Start Service", action: "start" });
});

test("Start Service through this install's unit runs systemctl and shows Starting", async () => {
  const subject = await service({
    files: { [UNIT_FILE]: `[Service]\nExecStart="${DAEMON_EXE}"\n` },
    answers: {
      "cd / && 'systemctl' '--user' 'start' 'allotd.service' < /dev/null > /dev/null 2>&1": {
        "exit code": 0,
        stdout: "",
      },
    },
  });
  subject.daemon.up = false;
  await subject.poll();

  const starting = subject.widget.startService();
  assert.equal(subject.widget.model.banner?.button, "starting");
  await starting;
  await settle();

  assert.ok(subject.commands.some((command) => command.includes("'systemctl' '--user' 'start'")));
  assert.ok(!subject.commands.some((command) => command.includes("setsid")));
  assert.equal(subject.widget.model.banner?.button, "starting");
});

test("without a unit Start Service runs allotd in its own session", async () => {
  const subject = await service({
    answers: { [`test -x '${DAEMON_EXE}'`]: { "exit code": 0, stdout: "" } },
  });
  subject.daemon.up = false;
  await subject.poll();

  await subject.widget.startService();
  await settle();

  assert.ok(
    subject.commands.includes(`cd / && setsid -f '${DAEMON_EXE}' < /dev/null > /dev/null 2>&1`),
  );
});

test("a missing allotd shows why in the banner", async () => {
  const subject = await service();
  subject.daemon.up = false;
  await subject.poll();

  await subject.widget.startService();
  await settle();

  const problem = `${DAEMON_EXE} is missing; run allot install`;
  assert.equal(subject.widget.model.banner?.button, "start");
  assert.equal(subject.widget.model.banner?.problem, problem);
  assert.ok(subject.lines.includes(`Allot: cannot start the service: ${problem}`));
});

test("Stop Service with jobs up asks in the popup, and Cancel keeps it running", async () => {
  const subject = await service();

  subject.widget.pressServiceItem();

  assert.equal(subject.expands, 1);
  assert.match(subject.widget.stopQuestion, /^2 jobs are running and 2 requests are waiting\./);
  assert.deepEqual(subject.posts, []);

  subject.widget.cancelStop();
  assert.equal(subject.widget.stopQuestion, "");
  assert.deepEqual(subject.posts, []);
});

test("closing the popup takes back the Stop question", async () => {
  const subject = await service();

  subject.widget.pressServiceItem();
  subject.widget.setExpanded(false);

  assert.equal(subject.widget.stopQuestion, "");
});

test("Stop Service's confirm stops allotd without a Service stopped notification", async () => {
  const subject = await service();

  subject.widget.pressServiceItem();
  subject.widget.confirmStop();
  await settle();
  subject.widget.setExpanded(false);
  await subject.poll();
  await subject.poll();
  await subject.poll();

  assert.deepEqual(subject.posts, [SHUTDOWN_URL]);
  assert.equal(subject.widget.model.status, "stopped");
  assert.deepEqual(subject.notifications, []);
});

test("Stop Service with no jobs stops without asking", async () => {
  const subject = await service({ fixture: "idle" });

  subject.widget.pressServiceItem();
  await settle();

  assert.equal(subject.expands, 0);
  assert.deepEqual(subject.posts, [SHUTDOWN_URL]);
});

test("a stop from outside notifies after two polls and its buttons work", async () => {
  const subject = await service({
    answers: { [`test -x '${DAEMON_EXE}'`]: { "exit code": 0, stdout: "" } },
  });
  subject.daemon.up = false;
  await subject.poll();
  assert.deepEqual(subject.notifications, [], "one stopped poll may be a restart");
  await subject.poll();

  const [sent] = subject.notifications;
  assert.equal(sent?.fields.title, "Service stopped");
  assert.equal(sent?.fields.icon, "allot-down-symbolic.svg");
  assert.deepEqual(
    sent?.fields.buttons.map((button) => button.label),
    ["Start Service", "Open Logs"],
  );

  sent?.press("Open Logs");
  assert.deepEqual(subject.opened, [`file://${HOME}/logs`]);

  sent?.press("Start Service");
  await settle();
  assert.ok(subject.commands.some((command) => command.includes("setsid -f")));

  subject.daemon.up = true;
  await subject.poll();
  assert.equal(sent?.closedByWidget, true, "the service is back, so the notification goes");
});

test("nothing notifies while the popup is open", async () => {
  const subject = await service();
  subject.widget.setExpanded(true);
  await settle();

  await stopFromOutside(subject);

  assert.deepEqual(subject.notifications, []);
});

test("a click on a notification opens the popup at its row", async () => {
  const subject = await service();
  await stopFromOutside(subject);

  subject.notifications[0]?.fields.activated();

  assert.equal(subject.expands, 1);
  assert.deepEqual(subject.scrolls, [null]);
});

test("a first run opens the popup once, shows the card and remembers it opened", async () => {
  const subject = await service({ fixture: "first-run", files: { [SETTINGS_FILE]: "{}" } });

  assert.equal(subject.expands, 1);
  assert.equal(subject.widget.model.settings.first_run_shown, true);
  assert.ok(subject.saves()[0]?.includes('"first_run_shown": true'));
  assert.ok(subject.rows.keys.includes("welcome"));

  subject.widget.setExpanded(false);
  await subject.poll();
  assert.equal(subject.expands, 1);
});

test("a first run that has opened before stays closed", async () => {
  const subject = await service({ fixture: "first-run" });

  assert.equal(subject.expands, 0);
  assert.equal(subject.widget.model.snapshot.welcome?.doNotDisturb, false);
});

test("Done ends the first run and brings back the empty line", async () => {
  const subject = await service({ fixture: "first-run" });
  subject.widget.setExpanded(true);
  await settle();
  assert.ok(subject.rows.keys.includes("welcome"));

  subject.widget.finishFirstRun();
  await settle();

  assert.equal(subject.rows.isLeaving("welcome"), true);
  assert.ok(subject.saves().at(-1)?.includes('"first_run_done": true'));
});

test("Do Not Disturb shows on the card without changing the count", async () => {
  const subject = await service({ fixture: "first-run", inhibited: "(<true>,)\n" });
  subject.widget.setExpanded(true);
  await settle();

  assert.equal(subject.widget.model.doNotDisturb, true);
  assert.equal(subject.widget.notificationCount, "3 of 5");
  assert.deepEqual(subject.shown.data.welcome, {
    key: "welcome",
    kind: "welcome",
    doNotDisturb: true,
    notificationsOff: false,
  });
});

test("notifications off in System Settings show on the card and in the count", async () => {
  const subject = await service({
    fixture: "first-run",
    files: { [`${CONFIG_HOME}/plasmanotifyrc`]: "[Services][allot]\nShowPopups=false\n" },
  });
  subject.widget.setExpanded(true);
  await settle();

  assert.equal(subject.widget.notificationsOff, true);
  assert.equal(subject.widget.notificationCount, "Off");
  assert.equal(/** @type {any} */ (subject.shown.data.welcome).notificationsOff, true);
});

test("the state is read again when More options asks, and a change shows", async () => {
  const eventFile = `${CONFIG_HOME}/allot.notifyrc`;
  const subject = await service({ files: { [eventFile]: "[Event/notification]\nAction=\n" } });
  assert.equal(subject.widget.notificationsOff, true);
  const changes = subject.changes;

  delete subject.files[eventFile];
  await subject.widget.refreshNotificationState();

  assert.equal(subject.widget.notificationCount, "3 of 5");
  assert.ok(subject.changes > changes);
});

test("Open Notification Settings starts System Settings on its own", async () => {
  const subject = await service();

  subject.widget.openNotificationSettings();

  assert.equal(
    subject.commands.at(-1),
    "cd / && setsid -f 'systemsettings' 'kcm_notifications' < /dev/null > /dev/null 2>&1",
  );
});
