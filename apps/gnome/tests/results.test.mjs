import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CancelledError,
  TimeoutError,
  conflictReason,
  pollResult,
  rethrowCancelled,
  runningState,
  settingsFailure,
  stoppedOrRethrow,
} from "../allot@mehran.tech/lib/results.js";
import { fixtureText } from "./support.mjs";

test("a cancel is passed on and anything else reads as stopped", () => {
  assert.throws(() => rethrowCancelled(new CancelledError()), CancelledError);
  assert.doesNotThrow(() => rethrowCancelled(new TimeoutError()));
  assert.throws(() => stoppedOrRethrow(new CancelledError()), CancelledError);
  assert.deepEqual(stoppedOrRethrow(new Error("gone")), pollResult("stopped", { detail: "gone" }));
});

test("a state that decodes is running, and one that does not is incompatible", () => {
  const running = runningState(fixtureText("ui-state-idle.json"));
  assert.equal(running.status, "running");
  assert.equal(running.version, running.state?.service.version);

  assert.equal(runningState("{}").status, "incompatible");
  assert.equal(runningState("not json").status, "incompatible");
});

test("a conflict names the reason after the lease", () => {
  assert.equal(conflictReason("lease 7 was not killed: its tree is protected"), "its tree is protected");
  assert.equal(conflictReason("lease 7 has already ended"), "the job has already ended");
  assert.equal(conflictReason(""), "the service refused");
});

test("a settings failure has no problems or revision", () => {
  assert.deepEqual(settingsFailure("held", "in use"), {
    ok: false,
    failure: { kind: "held", message: "in use", problems: [], revision: null },
  });
});
