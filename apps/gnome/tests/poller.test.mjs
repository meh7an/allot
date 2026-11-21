import assert from "node:assert/strict";
import { test } from "node:test";

import { BusClient } from "../allot@mehran.tech/lib/bus-client.js";
import { DaemonPoller } from "../allot@mehran.tech/lib/poller.js";
import { CancelledError } from "../allot@mehran.tech/lib/results.js";
import { FakeBus, FakeTimers, fixtureText, settle } from "./support.mjs";

/**
 * A poller of a daemon that serves the idle fixture while `daemon.up` says so and is not on the
 * bus otherwise.
 */
function poller(daemon = { up: true }) {
  const body = fixtureText("ui-state-idle.json");
  const transport = new FakeBus(() => {
    if (!daemon.up) {
      throw new Error("The name is not activatable");
    }
    return body;
  });
  const timers = new FakeTimers();
  return {
    poller: new DaemonPoller(new BusClient(transport.bus()), timers),
    timers,
    transport,
  };
}

test("running polls every five seconds closed and every second open", async () => {
  const { poller: subject } = poller();

  assert.equal(await subject.pollOnce(), 5000);
  subject.setPopupOpen(true);
  assert.equal(await subject.pollOnce(), 1000);
});

test("a stopped daemon backs off and keeps the last known state", async () => {
  const daemon = { up: true };
  const { poller: subject } = poller(daemon);
  /** @type {string[]} */
  const updates = [];
  subject.onUpdate = (result) => updates.push(result.status);
  await subject.pollOnce();

  daemon.up = false;
  const delays = [await subject.pollOnce(), await subject.pollOnce(), await subject.pollOnce()];

  assert.deepEqual(delays, [1000, 2000, 4000]);
  assert.equal(subject.latest?.status, "stopped");
  assert.equal(subject.lastKnown?.sessions.length, 2);
  assert.deepEqual(updates, ["running", "stopped", "stopped", "stopped"]);
});

test("opening the popup restarts the backoff", async () => {
  const { poller: subject } = poller({ up: false });
  await subject.pollOnce();
  await subject.pollOnce();

  subject.setPopupOpen(true);

  assert.equal(await subject.pollOnce(), 1000);
});

test("the loop waits on the schedule and pollNow wakes it early", async () => {
  const { poller: subject, timers, transport } = poller();
  const loop = subject.start();
  await settle();
  assert.equal(transport.calls.length, 1);
  assert.deepEqual(timers.delays, [5000]);

  subject.pollNow();
  await settle();

  assert.equal(transport.calls.length, 2);
  assert.deepEqual(timers.delays, [5000]);

  timers.fireAll();
  await settle();
  assert.equal(transport.calls.length, 3);

  subject.stop();
  await loop;
  assert.equal(timers.pending.size, 0);
});

test("a wake that arrives mid-poll skips the next wait", async () => {
  /** @type {() => void} */
  let answer = () => {};
  const body = fixtureText("ui-state-idle.json");
  const transport = new FakeBus(() => new Promise((resolve) => (answer = () => resolve(body))));
  const timers = new FakeTimers();
  const subject = new DaemonPoller(new BusClient(transport.bus()), timers);
  const loop = subject.start();
  await settle();

  subject.pollNow();
  answer();
  await settle();

  assert.equal(transport.calls.length, 2);
  assert.equal(timers.pending.size, 0);
  subject.stop();
  answer();
  await loop;
});

test("stop drops the poll in flight", async () => {
  /** @type {() => void} */
  let answer = () => {};
  const body = fixtureText("ui-state-idle.json");
  const transport = new FakeBus(() => new Promise((resolve) => (answer = () => resolve(body))));
  const subject = new DaemonPoller(new BusClient(transport.bus()), new FakeTimers());
  let updates = 0;
  subject.onUpdate = () => updates++;
  const loop = subject.start();
  await settle();

  subject.stop();
  answer();
  await loop;

  assert.equal(updates, 0);
  const late = subject.pollOnce();
  await settle();
  answer();
  await assert.rejects(late, CancelledError);
});

test("a cancelled request ends the loop", async () => {
  const transport = new FakeBus(() => {
    throw new CancelledError();
  });
  const subject = new DaemonPoller(new BusClient(transport.bus()), new FakeTimers());

  await subject.start();

  assert.equal(subject.latest, null);
});

test("an onUpdate that throws is reported once per message and polling goes on", async () => {
  const { poller: subject, timers, transport } = poller();
  /** @type {string[]} */
  const reported = [];
  let message = "card failed";
  subject.onUpdate = () => {
    throw new Error(message);
  };
  subject.onError = (error) => reported.push(String(error));
  const loop = subject.start();
  await settle();

  timers.fireAll();
  await settle();
  message = "popup failed";
  timers.fireAll();
  await settle();

  assert.equal(transport.calls.length, 3);
  assert.deepEqual(reported, ["Error: card failed", "Error: popup failed"]);
  assert.equal(subject.latest?.status, "running");
  subject.stop();
  await loop;
});

test("a message reported before is reported again after an update succeeds", async () => {
  const { poller: subject } = poller();
  /** @type {string[]} */
  const reported = [];
  let broken = true;
  subject.onUpdate = () => {
    if (broken) {
      throw new Error("card failed");
    }
  };
  subject.onError = (error) => reported.push(String(error));

  await subject.pollOnce();
  await subject.pollOnce();
  broken = false;
  await subject.pollOnce();
  broken = true;
  await subject.pollOnce();

  assert.equal(reported.length, 2);
});
