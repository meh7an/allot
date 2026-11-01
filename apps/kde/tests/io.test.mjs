import assert from "node:assert/strict";
import { test } from "node:test";

import { CancelledError, TimeoutError } from "../../gnome/allot@mehran.tech/lib/client.js";
import { FakeTimers, settle } from "../../gnome/tests/support.mjs";
import {
  CommandRunner,
  XhrTransport,
  catCommand,
  copyFile,
  detachedCommand,
  localPath,
  readTextFile,
  serviceSystem,
  shellQuote,
  writeTextFile,
} from "../src/io.js";
import { catAnswer, fakeRequests } from "./support.mjs";

/** @import { Answer, FakeXhr, SentRequest } from "./support.mjs" */

const FILES = { "/home/u/.allot/token": "secret\n" };

/** A runner whose engine answers from `FILES` once promises run. */
function runner() {
  /** @type {string[]} */
  const connected = [];
  const subject = new CommandRunner((command) => {
    connected.push(command);
    queueMicrotask(() => subject.deliver(command, catAnswer(command, FILES)));
  });
  return { subject, connected };
}

/** @param {(request: SentRequest) => Answer | undefined} answer */
function transport(answer) {
  const timers = new FakeTimers();
  /** @type {FakeXhr[]} */
  const requests = [];
  const subject = new XhrTransport(fakeRequests(answer, requests), timers);
  return { subject, timers, requests };
}

/** @param {(command: string) => number} exitCode */
function exitsWith(exitCode) {
  /** @type {string[]} */
  const commands = [];
  const subject = new CommandRunner((command) => {
    commands.push(command);
    queueMicrotask(() => subject.deliver(command, { "exit code": exitCode(command), stdout: "" }));
  });
  return { subject, commands };
}

test("a write goes through a temporary file, quoting every word", async () => {
  const { subject, commands } = exitsWith(() => 0);

  const written = await writeTextFile(subject, "/home/u s/.allot/tray.json", "it's\n");

  assert.equal(written, true);
  assert.deepEqual(commands, [
    "mkdir -p -- '/home/u s/.allot' && printf %s 'it'\\''s\n' > " +
      "'/home/u s/.allot/tray.json.tmp' && " +
      "mv -f -- '/home/u s/.allot/tray.json.tmp' '/home/u s/.allot/tray.json'",
  ]);
});

test("a failed write and a failed copy say so", async () => {
  const { subject } = exitsWith(() => 1);

  assert.equal(await writeTextFile(subject, "/a/b", "x"), false);
  assert.equal(await copyFile(subject, "/a/b", "/c/d"), false);
});

test("a copy skips an identical target and makes the folder", async () => {
  const { subject, commands } = exitsWith(() => 0);

  assert.equal(await copyFile(subject, "/pkg/a", "/data/k/a"), true);
  assert.deepEqual(commands, [
    "cmp -s -- '/pkg/a' '/data/k/a' || { mkdir -p -- '/data/k' && cp -f -- '/pkg/a' '/data/k/a'; }",
  ]);
});

test("the service system answers from exit codes, and a spawn does not wait", async () => {
  const { subject, commands } = exitsWith((command) =>
    command.startsWith("test -x '/ok'") || command.includes("'true'") ? 0 : 1,
  );
  const system = serviceSystem(subject);

  assert.equal(await system.isExecutable("/ok"), true);
  assert.equal(await system.isExecutable("/no"), false);
  assert.equal(await system.run(["true"]), true);
  assert.equal(await system.run(["false"]), false);
  system.spawn("/bin/allot d");
  assert.equal(commands.at(-1), detachedCommand(["/bin/allot d"]));
  assert.equal(commands.at(-1), "cd / && setsid -f '/bin/allot d' < /dev/null > /dev/null 2>&1");
});

const REQUEST = { method: /** @type {const} */ ("GET"), url: "http://127.0.0.1:1/ui/state" };

test("shell quoting keeps spaces and single quotes inside one word", () => {
  assert.equal(shellQuote("/home/a b/x"), "'/home/a b/x'");
  assert.equal(shellQuote("it's"), "'it'\\''s'");
  assert.equal(catCommand("/tmp/it's"), "cat -- '/tmp/it'\\''s'");
});

test("a file URL becomes its decoded local path", () => {
  assert.equal(localPath("file:///home/user%20one"), "/home/user one");
  assert.equal(localPath("/already/a/path"), "/already/a/path");
});

test("a readable file gives its text and a missing one null", async () => {
  const { subject } = runner();

  assert.equal(await readTextFile(subject, "/home/u/.allot/token"), "secret\n");
  assert.equal(await readTextFile(subject, "/home/u/.allot/config.toml"), null);
});

test("callers asking for the same command while it runs share one run", async () => {
  const { subject, connected } = runner();

  const both = await Promise.all([
    subject.run(catCommand("/home/u/.allot/token")),
    subject.run(catCommand("/home/u/.allot/token")),
  ]);

  assert.equal(connected.length, 1);
  assert.deepEqual(both[0], { exitCode: 0, stdout: "secret\n" });
  assert.deepEqual(both[1], both[0]);
  await subject.run(catCommand("/home/u/.allot/token"));
  assert.equal(connected.length, 2, "a finished command runs again");
});

test("a request sends the bearer token and resolves with status and body", async () => {
  const { subject, timers, requests } = transport(() => ({ status: 200, body: "{}" }));

  const response = await subject.send({ ...REQUEST, token: "t", timeoutSecs: 3 });

  assert.deepEqual(response, { status: 200, body: "{}" });
  assert.deepEqual(requests[0]?.sent, {
    method: "GET",
    url: REQUEST.url,
    headers: { Authorization: "Bearer t" },
  });
  assert.equal(timers.pending.size, 0, "the timeout is cleared");
});

test("a request without a token sends no Authorization header", async () => {
  const { subject, requests } = transport(() => ({ status: 200, body: "" }));

  await subject.send({ ...REQUEST, token: null, timeoutSecs: 3 });

  assert.deepEqual(requests[0]?.sent.headers, {});
});

test("a refused connection rejects with an error that is not a cancel", async () => {
  const { subject } = transport(() => null);

  await assert.rejects(subject.send({ ...REQUEST, token: "t", timeoutSecs: 3 }), (error) => {
    assert.ok(error instanceof Error && !(error instanceof CancelledError));
    assert.match(error.message, /nothing answers/);
    return true;
  });
});

test("a request without an answer is aborted after its timeout", async () => {
  const { subject, timers, requests } = transport(() => undefined);

  const sent = subject.send({ ...REQUEST, token: "t", timeoutSecs: 3 });
  assert.deepEqual(timers.delays, [3000]);
  timers.fireAll();

  await assert.rejects(sent, TimeoutError);
  assert.equal(requests[0]?.aborted, true);
});

test("cancel rejects requests in flight with CancelledError and clears timeouts", async () => {
  const { subject, timers, requests } = transport(() => undefined);

  const sent = subject.send({ ...REQUEST, token: "t", timeoutSecs: 3 });
  subject.cancel();

  await assert.rejects(sent, CancelledError);
  assert.equal(requests[0]?.aborted, true);
  assert.equal(timers.pending.size, 0);
  await settle();
});
