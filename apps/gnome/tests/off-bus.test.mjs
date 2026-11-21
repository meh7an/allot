import assert from "node:assert/strict";
import { test } from "node:test";

import { allotdAliveOffBus, lockPid } from "../allot@mehran.tech/lib/off-bus.js";

const ROOT = "/home/user/.allot";

/**
 * A reader over a few files; it records what it was asked for.
 *
 * @param {Record<string, string>} files
 */
function reader(files) {
  /** @type {string[]} */
  const asked = [];
  return {
    asked,
    read: async (/** @type {string} */ path) => {
      asked.push(path);
      return files[path] ?? null;
    },
  };
}

test("the lock file's process id is a whole number on its own line", () => {
  assert.equal(lockPid("4242\n"), 4242);
  assert.equal(lockPid("  17  "), 17);
  assert.equal(lockPid(""), null);
  assert.equal(lockPid(null), null);
  assert.equal(lockPid("0\n"), null);
  assert.equal(lockPid("12 34"), null);
  assert.equal(lockPid("-5"), null);
  assert.equal(lockPid("pid 5"), null);
});

test("allotd is alive when the lock names a process called allotd", async () => {
  const files = reader({ [`${ROOT}/allotd.lock`]: "4242\n", "/proc/4242/comm": "allotd\n" });

  assert.equal(await allotdAliveOffBus(ROOT, files.read), true);
  assert.deepEqual(files.asked, [`${ROOT}/allotd.lock`, "/proc/4242/comm"]);
});

test("a reused process id, a gone process or a missing lock is not allotd", async () => {
  const reused = reader({ [`${ROOT}/allotd.lock`]: "4242\n", "/proc/4242/comm": "firefox\n" });
  assert.equal(await allotdAliveOffBus(ROOT, reused.read), false);

  const gone = reader({ [`${ROOT}/allotd.lock`]: "4242\n" });
  assert.equal(await allotdAliveOffBus(ROOT, gone.read), false);

  const missing = reader({});
  assert.equal(await allotdAliveOffBus(ROOT, missing.read), false);
  assert.deepEqual(missing.asked, [`${ROOT}/allotd.lock`]);
});

test("an older allotd leaves the lock empty, which reads as not alive", async () => {
  const old = reader({ [`${ROOT}/allotd.lock`]: "" });

  assert.equal(await allotdAliveOffBus(ROOT, old.read), false);
  assert.deepEqual(old.asked, [`${ROOT}/allotd.lock`]);
});
