import assert from "node:assert/strict";
import { test } from "node:test";

import {
  EXPECTED_STOP_WINDOW_MS,
  MERGE_WINDOW_MS,
  NotificationRules,
} from "../allot@mehran.tech/lib/notification-rules.js";
import { defaultSettings } from "../allot@mehran.tech/lib/settings.js";
import { NOW_MS, decodeState, scenarioJson, sessionOf, leaseId } from "./handoff.mjs";

/** @import { NotificationToggles } from "../allot@mehran.tech/lib/settings.js" */
/** @import { ServiceStatus } from "../allot@mehran.tech/lib/client.js" */
/** @import { UiState } from "../allot@mehran.tech/contract.g.mjs" */

const ALL_ON = {
  service_stopped: true,
  running_long: true,
  benchmark_finished: true,
  job_finished: true,
  killed_outside_flyout: true,
};

class Harness {
  nowMs = 1000;
  rules = new NotificationRules(() => this.nowMs);
  /** @type {UiState | null} */
  lastKnown = null;
  /** @type {NotificationToggles} */
  toggles = ALL_ON;
  popupOpen = false;

  /**
   * @param {ServiceStatus} status
   * @param {UiState | null} [live]
   */
  poll(status, live = null) {
    if (live !== null) {
      this.lastKnown = live;
    }
    return this.rules.observe({
      status,
      live,
      lastKnown: this.lastKnown,
      toggles: this.toggles,
      popupOpen: this.popupOpen,
    });
  }
}

/**
 * @param {(json: any) => void} [change]
 * @param {string} [name]
 * @returns {UiState}
 */
function state(change = () => {}, name = "empty") {
  const { json } = scenarioJson(name);
  json.recent = [];
  change(json);
  return decodeState(json);
}

/**
 * @param {number} lease
 * @param {string} command
 * @param {{ noise?: object | null, cause?: string | null }} [options]
 * @returns {object}
 */
function run(lease, command, options = {}) {
  return {
    command,
    cwd: "/home/user/code/app",
    declared: { cores: 1, ram_mb: 1, gpu_pct: 0, vram_mb: 0, est_secs: 60 },
    end_cause: options.cause === undefined ? "released" : options.cause,
    ended_at_ms: NOW_MS - 1000,
    lease_id: lease,
    noise: options.noise ?? null,
    session: sessionOf("cargo"),
    started_at_ms: NOW_MS - 293_000,
  };
}

/**
 * @param {number} lease
 * @param {string} command
 * @param {string} [cause]
 * @returns {object}
 */
function notice(lease, command, cause = "killed") {
  return {
    cause,
    command,
    delivered_at_ms: null,
    ended_at_ms: NOW_MS,
    lease_id: lease,
    session: sessionOf("cargo"),
    tool_use_id: null,
  };
}

const NOISE = {
  avg_cpu_pct: 1.8,
  avg_gpu_pct: 0,
  peak_cpu_pct: 6.2,
  peak_gpu_pct: 0,
  quiet_at_start: true,
};

test("the first live state is a baseline, so a restart repeats nothing", () => {
  const harness = new Harness();

  const update = harness.poll(
    "running",
    state((json) => {
      json.recent = [run(1, "cargo build")];
      json.notices = [notice(2, "cargo test")];
    }, "c-over"),
  );

  assert.deepEqual(update, { show: [], withdraw: [] });
});

test("a job past 1.5 times its estimate notifies once, with Show", () => {
  const harness = new Harness();
  harness.poll("running", state());

  // c-over's job is at 700 s against a 600 s estimate.
  const notLong = harness.poll("running", state(() => {}, "c-over"));
  assert.deepEqual(notLong.show, []);

  const long = state((json) => {
    json.generated_at_ms = NOW_MS + 300_000;
  }, "c-over");
  const update = harness.poll("running", long);

  assert.equal(update.show.length, 1);
  const [message] = update.show;
  assert.equal(message?.title, "python train.py --epochs 20 --data ./shards is running long");
  assert.equal(message?.body, "16:40 so far against a ~10:00 estimate · b81d44f0 · ml-trainer");
  assert.deepEqual(message?.buttons, ["show"]);
  assert.equal(message?.target, `job:${leaseId("py")}`);

  assert.deepEqual(harness.poll("running", long).show, []);
});

test("a finished job notifies only with its toggle on, and several merge within a minute", () => {
  const harness = new Harness();
  harness.poll("running", state());

  const one = harness.poll(
    "running",
    state((json) => {
      json.recent = [run(1, "cargo build")];
    }),
  );
  assert.equal(one.show[0]?.title, "cargo build finished");
  assert.equal(one.show[0]?.body, "4:52 · 3f2a9c1e · app");
  assert.equal(one.show[0]?.target, "recent");
  assert.equal(one.show[0]?.tag, "done-1");

  harness.nowMs += 1000;
  const two = harness.poll(
    "running",
    state((json) => {
      json.recent = [run(2, "cargo test"), run(1, "cargo build")];
    }),
  );
  assert.equal(two.show[0]?.title, "2 jobs finished");
  assert.equal(two.show[0]?.body, "cargo build and cargo test");
  assert.equal(two.show[0]?.tag, "done-1");

  harness.nowMs += 1000;
  const three = harness.poll(
    "running",
    state((json) => {
      json.recent = [run(3, "go test"), run(2, "cargo test"), run(1, "cargo build")];
    }),
  );
  assert.equal(three.show[0]?.title, "3 jobs finished");
  assert.equal(three.show[0]?.body, "cargo build, cargo test and 1 more");

  harness.nowMs += MERGE_WINDOW_MS;
  const later = harness.poll(
    "running",
    state((json) => {
      json.recent = [
        run(4, "make"),
        run(3, "go test"),
        run(2, "cargo test"),
        run(1, "cargo build"),
      ];
    }),
  );
  assert.equal(later.show[0]?.title, "make finished");
  assert.equal(later.show[0]?.tag, "done-2");
});

test("the defaults stay quiet about finished jobs and kills from outside", () => {
  const harness = new Harness();
  harness.toggles = defaultSettings().notifications;
  harness.poll("running", state());

  const update = harness.poll(
    "running",
    state((json) => {
      json.recent = [run(1, "cargo build")];
      json.notices = [notice(2, "cargo test")];
    }),
  );

  assert.deepEqual(update.show, []);
});

test("a killed run does not read as finished", () => {
  const harness = new Harness();
  harness.poll("running", state());

  const update = harness.poll(
    "running",
    state((json) => {
      json.recent = [run(1, "cargo build", { cause: "killed" })];
    }),
  );

  assert.deepEqual(update.show, []);
});

test("a benchmark says its noise, and when it was not quiet at the start", () => {
  const harness = new Harness();
  harness.poll("running", state());

  const update = harness.poll(
    "running",
    state((json) => {
      json.recent = [
        run(8, "cargo bench", { noise: { ...NOISE, quiet_at_start: false } }),
        run(7, "cargo bench", { noise: NOISE }),
      ];
    }),
  );

  assert.deepEqual(
    update.show.map((message) => [message.title, message.body, message.tag]),
    [
      [
        "Benchmark finished in 4:52",
        "Background noise avg 1.8%, peak 6.2% CPU · cargo bench",
        "bench-7",
      ],
      [
        "Benchmark finished in 4:52",
        "Background noise avg 1.8%, peak 6.2% CPU · not quiet at start · cargo bench",
        "bench-8",
      ],
    ],
  );
});

test("a kill from outside notifies, a kill from the popup and a removal do not", () => {
  const harness = new Harness();
  harness.poll("running", state());
  harness.rules.noteLocalKill(3);

  const update = harness.poll(
    "running",
    state((json) => {
      json.notices = [notice(4, "cargo test", "removed"), notice(3, "npm"), notice(2, "cargo")];
    }),
  );

  assert.deepEqual(
    update.show.map((message) => [message.title, message.body, message.buttons]),
    [["cargo was stopped", "Killed from the command line · Claude was notified", []]],
  );
});

test("the open popup stays quiet, and closing it brings nothing later", () => {
  const harness = new Harness();
  harness.poll("running", state());
  harness.popupOpen = true;

  const finished = state((json) => {
    json.recent = [run(1, "cargo build")];
    json.notices = [notice(2, "cargo")];
  });
  assert.deepEqual(harness.poll("running", finished).show, []);

  harness.popupOpen = false;
  assert.deepEqual(harness.poll("running", finished).show, []);
});

test("a stop needs two stopped polls, says the sessions, and goes when the service is back", () => {
  const harness = new Harness();
  const live = state((json) => {
    json.sessions = scenarioJson("several").json.sessions;
  });
  harness.poll("running", live);

  assert.deepEqual(harness.poll("stopped").show, []);
  const update = harness.poll("stopped");
  assert.equal(update.show[0]?.title, "Service stopped");
  assert.equal(
    update.show[0]?.body,
    "Heavy commands now run unchecked. 4 Claude sessions are connected.",
  );
  assert.deepEqual(update.show[0]?.buttons, ["start-service", "open-logs"]);
  assert.deepEqual(harness.poll("stopped").show, []);

  assert.deepEqual(harness.poll("running", live).withdraw, ["service"]);
  assert.deepEqual(harness.poll("running", live).withdraw, []);
});

test("one stopped poll is a restart, and starting or incompatible are not stops", () => {
  const harness = new Harness();
  harness.poll("running", state());

  harness.poll("stopped");
  harness.poll("running", state());
  assert.deepEqual(harness.poll("stopped").show, []);

  harness.poll("running", state());
  harness.poll("starting");
  harness.poll("incompatible");
  assert.deepEqual(harness.poll("starting").show, []);
});

test("a stopped service never seen running says nothing", () => {
  const harness = new Harness();

  harness.poll("stopped");
  assert.deepEqual(harness.poll("stopped").show, []);
});

test("a stop the user chose says nothing, unless it comes much later", () => {
  const chosen = new Harness();
  chosen.poll("running", state());
  chosen.rules.expectStop();
  chosen.poll("stopped");
  assert.deepEqual(chosen.poll("stopped").show, []);

  const late = new Harness();
  late.poll("running", state());
  late.rules.expectStop();
  late.nowMs += EXPECTED_STOP_WINDOW_MS + 1;
  late.poll("stopped");
  assert.equal(late.poll("stopped").show.length, 1);
});

test("Service stopped follows its toggle", () => {
  const harness = new Harness();
  harness.toggles = { ...ALL_ON, service_stopped: false };
  harness.poll("running", state());
  harness.poll("stopped");

  assert.deepEqual(harness.poll("stopped").show, []);
  assert.deepEqual(harness.poll("running", state()).withdraw, []);
});
