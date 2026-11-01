import assert from "node:assert/strict";
import { test } from "node:test";

import { FakeTimers, fixtureText, settle } from "../../gnome/tests/support.mjs";
import { NOW_MS, leaseId, scenarioJson } from "../../gnome/tests/handoff.mjs";
import { DESKTOP_ENTRIES_COMMAND } from "../src/app-icons.js";
import { GROUP_ICONS, groupIcon, idleText } from "../src/widget.js";
import { fakeHost } from "./support.mjs";

const HOME = "/home/user one/.allot";
const TOKEN_FILE = `${HOME}/token`;
const CONFIG_FILE = `${HOME}/config.toml`;

/**
 * A widget whose daemon serves `fixture` on the given port.
 *
 * @param {{ fixture?: string, files?: Record<string, string>, port?: number }} [options]
 */
function started(options = {}) {
  const timers = new FakeTimers();
  const body = fixtureText(`ui-state-${options.fixture ?? "busy"}.json`);
  const port = options.port ?? 47823;
  const host = fakeHost({
    timers,
    files: options.files ?? { [TOKEN_FILE]: "t\n" },
    daemon: (request) => {
      if (!request.url.startsWith(`http://127.0.0.1:${port}/`)) {
        return null;
      }
      return request.headers.Authorization === "Bearer t" ? { status: 200, body } : null;
    },
  });
  host.widget.start();
  return host;
}

test("the data paths come from the home URL", () => {
  const { widget } = fakeHost({ timers: new FakeTimers(), daemon: () => null });

  assert.equal(widget.paths.token, TOKEN_FILE);
  assert.equal(widget.paths.logs, `${HOME}/logs`);
});

test("starting reads the config, then the token, and shows the first poll", async () => {
  const subject = started();
  await settle();

  assert.deepEqual(subject.commands, [
    `cat -- '${CONFIG_FILE}'`,
    `cat -- '${TOKEN_FILE}'`,
  ]);
  assert.equal(subject.requests[0]?.sent.url, "http://127.0.0.1:47823/ui/state");
  assert.equal(subject.changes, 1);
  assert.equal(subject.widget.model.snapshot.header.summary, "2 running · 2 waiting");
  assert.equal(subject.widget.iconFile, "allot-queued-symbolic.svg");
  assert.equal(subject.widget.accessibleName, "Allot · 2 running · 2 waiting");
  assert.deepEqual(subject.lines, ["Allot: service running: 0.2.0"]);
  assert.deepEqual(subject.timers.delays, [5000], "the closed popup polls every 5 s");
});

test("the port comes from config.toml when it sets one", async () => {
  const subject = started({
    port: 47901,
    files: { [TOKEN_FILE]: "t", [CONFIG_FILE]: "[daemon]\nport = 47_901\n" },
  });
  await settle();

  assert.equal(subject.requests[0]?.sent.url, "http://127.0.0.1:47901/ui/state");
  assert.equal(subject.widget.model.snapshot.header.summary, "2 running · 2 waiting");
});

test("without a token the service reads as stopped and nothing is requested", async () => {
  const subject = started({ files: {} });
  await settle();

  assert.equal(subject.requests.length, 0);
  assert.equal(subject.widget.model.snapshot.header.summary, "Service stopped");
  assert.equal(subject.widget.iconFile, "allot-down-symbolic.svg");
  assert.deepEqual(subject.lines, ["Allot: service stopped: no token in ~/.allot/token"]);
});

test("an unchanged state calls onChange and logs only once", async () => {
  const subject = started();
  await settle();
  subject.timers.fireAll();
  await settle();

  assert.equal(subject.requests.length, 2);
  assert.equal(subject.changes, 1);
  assert.equal(subject.lines.length, 1);
});

test("opening polls at once on the 1 s cadence and starts without Keep Open", async () => {
  const subject = started();
  await settle();
  subject.widget.setExpanded(true);
  subject.widget.toggleKeepOpen();
  assert.equal(subject.widget.keepOpen, true);
  await settle();

  assert.equal(subject.requests.length, 2, "opening polled without waiting 5 s");
  assert.deepEqual(subject.timers.delays, [1000, 1000], "the poll's wait and the card tick");

  subject.widget.setExpanded(false);
  assert.equal(subject.widget.keepOpen, true, "closing alone keeps the mode until reopened");
  subject.widget.setExpanded(true);
  assert.equal(subject.widget.keepOpen, false);
});

test("Keep Open toggles", () => {
  const { widget } = fakeHost({ timers: new FakeTimers(), daemon: () => null });

  widget.toggleKeepOpen();
  assert.equal(widget.keepOpen, true);
  widget.toggleKeepOpen();
  assert.equal(widget.keepOpen, false);
});

test("stop ends polling and cancels the request in flight", async () => {
  const timers = new FakeTimers();
  const subject = fakeHost({
    timers,
    files: { [TOKEN_FILE]: "t" },
    daemon: () => undefined,
  });
  const running = subject.widget.start();
  await settle();
  assert.equal(subject.requests.length, 1);

  subject.widget.stop();
  await running;

  assert.equal(subject.requests[0]?.aborted, true);
  assert.equal(timers.pending.size, 0);
  assert.equal(subject.changes, 0, "a cancelled poll never reads as a stopped service");
});

test("stopping while the config is read never starts polling", async () => {
  const subject = fakeHost({ timers: new FakeTimers(), daemon: () => null });

  const running = subject.widget.start();
  subject.widget.stop();
  await running;

  assert.deepEqual(subject.commands, [`cat -- '${CONFIG_FILE}'`]);
  assert.equal(subject.requests.length, 0);
});

test("the idle line separates its parts with two spaces", () => {
  const idle = { label: "Idle · 2", ids: "aaaa  bbbb", usage: "" };

  assert.equal(idleText(idle), "Idle · 2  aaaa  bbbb");
});

test("each group has a Breeze icon", () => {
  assert.equal(groupIcon("benchmark"), GROUP_ICONS.benchmark);
  assert.equal(groupIcon("running"), "media-playback-start");
  assert.equal(groupIcon("waiting"), GROUP_ICONS.waiting);
});

/**
 * An open widget against a daemon serving a handoff scenario, on a clock the test moves. Kill and
 * Remove end the lease as allotd would, or answer `refusal` with a 409.
 *
 * @param {string} name
 * @param {{ refusal?: string, desktopEntries?: string, stalls?: boolean }} [options]
 */
async function opened(name, options = {}) {
  const timers = new FakeTimers();
  const { json } = scenarioJson(name);
  const state = /** @type {any} */ (json);
  const clock = { ms: NOW_MS };
  const subject = fakeHost({
    timers,
    now: () => clock.ms,
    files: { [TOKEN_FILE]: "t" },
    ...(options.desktopEntries === undefined ? {} : { desktopEntries: options.desktopEntries }),
    daemon: (request) => {
      if (request.method === "GET") {
        return { status: 200, body: JSON.stringify({ ...state, generated_at_ms: clock.ms }) };
      }
      if (options.stalls === true) {
        return undefined;
      }
      if (options.refusal !== undefined) {
        return { status: 409, body: `lease 1 was not killed: ${options.refusal}` };
      }
      const id = Number(/leases\/(\d+)\//.exec(request.url)?.[1]);
      const listed = [...state.active, ...state.queue].find((job) => job.lease.id === id);
      state.active = state.active.filter((/** @type {any} */ job) => job.lease.id !== id);
      state.queue = state.queue.filter((/** @type {any} */ job) => job.lease.id !== id);
      const lease = { ...listed.lease, state: "expired", ended_at_ms: clock.ms };
      const body = request.url.endsWith("/kill")
        ? { failed: [], lease, spared: [], terminated: [], untouched_reason: null }
        : { lease };
      return { status: 200, body: JSON.stringify(body) };
    },
  });
  subject.widget.start();
  await settle();
  subject.widget.setExpanded(true);
  await settle();
  /** @param {number} lease */
  const cardOf = (lease) => /** @type {any} */ (subject.shown.data[`job:${lease}`]).job.card;
  return {
    ...subject,
    state,
    cardOf,
    /** @param {number} ms */
    advance: (ms) => {
      clock.ms += ms;
    },
    posts: () => subject.requests.filter((request) => request.sent.method === "POST"),
  };
}

test("opening shows the rows at once, without animation", async () => {
  const subject = await opened("several");

  assert.deepEqual(subject.rows.keys.slice(0, 2), ["group:running", `job:${leaseId("cargo")}`]);
  assert.equal(subject.rows.get(0).entering, false);
});

test("while closed the list is left alone and no tick runs", async () => {
  const timers = new FakeTimers();
  const body = fixtureText("ui-state-busy.json");
  const subject = fakeHost({
    timers,
    files: { [TOKEN_FILE]: "t" },
    daemon: () => ({ status: 200, body }),
  });
  subject.widget.start();
  await settle();

  assert.equal(subject.rows.count, 0);
  assert.deepEqual(timers.delays, [5000]);
});

test("the tick updates times every second and keeps every view", async () => {
  const subject = await opened("one");
  const lease = leaseId("cargo");
  const inserted = subject.rows.inserted;
  const before = subject.cardOf(lease).time;

  subject.advance(1000);
  subject.timers.fireAll();
  await settle();

  assert.notEqual(subject.cardOf(lease).time, before);
  assert.equal(subject.rows.inserted, inserted);
  assert.equal(subject.shown.animate, true);
});

test("Kill asks first, ignores a double press, and sends on a later press", async () => {
  const subject = await opened("one");
  const lease = leaseId("cargo");

  subject.widget.pressKill(lease);
  assert.equal(subject.cardOf(lease).action, "confirm");
  assert.match(subject.cardOf(lease).detail, /^Stops cargo and its child processes/);
  assert.equal(subject.cardOf(lease).time, "4s");

  subject.advance(200);
  subject.widget.pressKill(lease);
  assert.equal(subject.posts().length, 0, "a double press within 300 ms is ignored");

  subject.advance(300);
  subject.widget.pressKill(lease);
  assert.equal(subject.cardOf(lease).isSending, true);
  assert.equal(subject.cardOf(lease).action, "none");
  await settle();

  assert.equal(subject.posts()[0]?.sent.url, `http://127.0.0.1:47823/leases/${lease}/kill`);
  assert.equal(subject.cardOf(lease).strong, "Stopped");
  assert.equal(subject.cardOf(lease).isStruck, true);
  assert.equal(subject.widget.model.snapshot.header.summary, "Idle");
});

test("answering a kill polls at once, without waiting for the 1 s cadence", async () => {
  const subject = await opened("one");
  const lease = leaseId("cargo");
  subject.widget.pressKill(lease);
  subject.advance(500);
  const polls = () => subject.requests.filter((request) => request.sent.method === "GET").length;
  const before = polls();

  subject.widget.pressKill(lease);
  await settle();

  assert.equal(polls(), before + 1);
});

test("a stopped card holds 4 s, then its row leaves and goes", async () => {
  const subject = await opened("one");
  const lease = leaseId("cargo");
  subject.widget.pressKill(lease);
  subject.advance(500);
  subject.widget.pressKill(lease);
  await settle();
  assert.equal(subject.rows.isLeaving(`job:${lease}`), false);

  subject.advance(4000);
  subject.timers.fireAll();
  await settle();

  assert.equal(subject.rows.isLeaving(`job:${lease}`), true);
  subject.widget.rowGone(`job:${lease}`);
  assert.equal(subject.rows.keys.includes(`job:${lease}`), false);
});

test("Kill? goes back after 4 s, on Esc, on focus leaving and on closing", async () => {
  const subject = await opened("one");
  const lease = leaseId("cargo");

  subject.widget.pressKill(lease);
  subject.advance(4000);
  subject.timers.fireAll();
  assert.equal(subject.cardOf(lease).action, "kill", "the countdown ended");

  subject.widget.pressKill(lease);
  subject.widget.cancelConfirms();
  assert.equal(subject.cardOf(lease).action, "kill", "Esc");

  subject.widget.pressKill(lease);
  subject.widget.cancelConfirm(lease);
  assert.equal(subject.cardOf(lease).action, "kill", "focus left");

  subject.widget.pressKill(lease);
  subject.widget.setExpanded(false);
  subject.widget.setExpanded(true);
  assert.equal(subject.cardOf(lease).action, "kill", "closed and opened");
  assert.equal(subject.posts().length, 0);
});

test("a refusal reads in red and the card keeps its Kill", async () => {
  const subject = await opened("one", { refusal: "it is protected" });
  const lease = leaseId("cargo");
  subject.widget.pressKill(lease);
  subject.advance(500);

  subject.widget.pressKill(lease);
  await settle();

  assert.equal(subject.cardOf(lease).detail, "Not stopped: it is protected");
  assert.equal(subject.cardOf(lease).detailIsDanger, true);
  assert.equal(subject.cardOf(lease).action, "kill");
});

test("a kill without an answer says so after its own 10 s", async () => {
  const subject = await opened("one", { stalls: true });
  const lease = leaseId("cargo");
  subject.widget.pressKill(lease);
  subject.advance(500);

  subject.widget.pressKill(lease);
  await settle();
  assert.ok(subject.timers.delays.includes(10_000));
  subject.timers.fireAll();
  await settle();

  assert.equal(
    subject.cardOf(lease).detail,
    "Not stopped: the service did not answer within 10 s",
  );
});

test("Remove sends at the first press and holds Removed", async () => {
  const subject = await opened("c-queued");
  const lease = leaseId("npm");

  subject.widget.pressRemove(lease);
  assert.equal(subject.cardOf(lease).detail, "Removing…");
  await settle();

  assert.equal(subject.posts()[0]?.sent.url, `http://127.0.0.1:47823/leases/${lease}/remove`);
  assert.equal(subject.cardOf(lease).strong, "Removed");
  assert.equal(subject.widget.model.nextStopChangeMs, 4000);
});

test("after closing, a tick still runs for a card's hold", async () => {
  const subject = await opened("c-queued");
  subject.widget.pressRemove(leaseId("npm"));
  await settle();
  subject.widget.setExpanded(false);
  await settle();

  assert.ok(subject.timers.delays.includes(4000));
});

test("the desktop entries are read once per opening and give the card its icon", async () => {
  const subject = await opened("icons", {
    desktopEntries: [
      "--",
      "./vim.desktop:Type=Application",
      "./vim.desktop:Exec=vim %F",
      "./vim.desktop:Icon=gvim",
      "",
    ].join("\n"),
  });
  await settle();

  const reads = () =>
    subject.commands.filter((command) => command === DESKTOP_ENTRIES_COMMAND).length;
  const icons = Object.values(subject.shown.data).flatMap((row) =>
    row.kind === "job" && row.icon !== null ? [row.icon] : [],
  );
  assert.equal(reads(), 1);
  assert.deepEqual(icons, ["gvim"]);

  subject.widget.setExpanded(false);
  subject.widget.setExpanded(true);
  await settle();
  assert.equal(reads(), 2);
});

