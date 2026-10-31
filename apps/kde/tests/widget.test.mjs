import assert from "node:assert/strict";
import { test } from "node:test";

import { FakeTimers, fixtureText, settle } from "../../gnome/tests/support.mjs";
import { GROUP_ICONS, groupIcon, idleText, stripText } from "../src/widget.js";
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
  assert.deepEqual(subject.timers.delays, [1000]);

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

test("a job's session line leaves out empty parts", () => {
  const strip = { shortId: "a1b2c3d4", project: "app", projectPath: "/w/app", usage: "" };

  assert.equal(stripText(strip), "a1b2c3d4 · app");
  assert.equal(
    stripText({ ...strip, usage: "1.5 cores · 2.0 GB" }),
    "a1b2c3d4 · app · 1.5 cores · 2.0 GB",
  );
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
