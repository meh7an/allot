import { decodeUiState } from "../allot@mehran.tech/contract.g.mjs";

/** @import { RecentRun, UiState } from "../allot@mehran.tech/contract.g.mjs" */
/** @import { ServiceStatus } from "../allot@mehran.tech/lib/client.js" */
/** @import { StopView } from "../allot@mehran.tech/lib/job-stops.js" */

// The sample jobs of the handoff prototype (`JOBS` in `Flyout.dc.html`) as daemon state, so
// that each `SCEN` scenario can be rebuilt from what allotd would send: the macOS app's
// `Handoff` with Linux paths and process names. The tests check the logic against it, and the
// manual check's fixture server serves it.

export const NOW_MS = 1_792_824_720_000;

export const IDLE_SESSION = "e09c7b3a-1111-4000-8000-000000000000";

/** The scenarios of the state board that show jobs, in its order. */
export const SCENARIO_NAMES = [
  "empty",
  "one",
  "several",
  "kill-confirm",
  "killed",
  "bench-drain",
  "bench-quiet",
  "bench-run",
  "bench-done",
  "bench-noisy",
  "bench-forced",
  "down",
  "max",
  "icons",
  "c-active",
  "c-over",
  "c-queued",
  "c-bench",
  "c-confirm",
  "c-killed",
  "c-removed",
  "c-refused",
];

/** The scenario whose Kill the fixture server refuses, to show the inline error. */
export const REFUSED_SCENARIO = "c-refused";

/** The refusal of `REFUSED_SCENARIO`, as `DaemonClient` words a 409. */
export const REFUSED_REASON = "the bound process 18244 (cargo) is protected";

// A week before NOW_MS.
const FIRST_SESSION_MS = NOW_MS - 7 * 86_400_000;

/**
 * @typedef {object} Profile
 * @property {number} cores
 * @property {number} ramMb
 * @property {number} [gpuPct]
 * @property {number} [vramMb]
 */

/**
 * `elapsed` and `estimate` are an active job's seconds since its grant and its estimate;
 * `waited` and `waitLeft` a queued job's seconds since its request and allotd's remaining wait.
 *
 * @typedef {object} Job
 * @property {number} id
 * @property {string} session
 * @property {string} cwd
 * @property {number} cores
 * @property {number} ramMb
 * @property {string} command
 * @property {string | null} jobClass
 * @property {string | null} process
 * @property {number} pid
 * @property {Profile} profile
 * @property {number} elapsed
 * @property {number} estimate
 * @property {number} waited
 * @property {number | null} waitLeft
 */

/**
 * @param {Partial<Job> & Pick<Job, "id" | "session" | "cwd" | "cores" | "ramMb" | "command" |
 *   "profile">} job
 * @returns {Job}
 */
function job(job) {
  return {
    jobClass: "cpu_heavy",
    process: null,
    pid: 0,
    elapsed: 0,
    estimate: 300,
    waited: 0,
    waitLeft: null,
    ...job,
  };
}

/** @type {Record<string, Job>} */
const JOBS = {
  cargo: job({
    id: 1,
    session: "3f2a9c1e",
    cwd: "/home/user/code/allot",
    cores: 7.8,
    ramMb: 2970,
    command: "cargo build --release",
    process: "cargo",
    pid: 18244,
    profile: { cores: 8, ramMb: 4096 },
    elapsed: 186,
    estimate: 300,
  }),
  py: job({
    id: 2,
    session: "b81d44f0",
    cwd: "/home/user/code/ml-trainer",
    cores: 1.9,
    ramMb: 6554,
    command: "python train.py --epochs 20 --data ./shards",
    jobClass: "gpu_heavy",
    process: "python3.13",
    pid: 22108,
    profile: { cores: 2, ramMb: 4096, gpuPct: 80, vramMb: 4096 },
    elapsed: 700,
    estimate: 600,
  }),
  vite: job({
    id: 3,
    session: "5d2e8b61",
    cwd: "/home/user/code/design-tokens-and-a-very-long-folder-name",
    cores: 3.2,
    ramMb: 1843,
    command: "pnpm --filter web vite build --mode production",
    jobClass: "mixed",
    process: "node",
    pid: 9120,
    profile: { cores: 4, ramMb: 2048, gpuPct: 10, vramMb: 1024 },
    elapsed: 52,
    estimate: 90,
  }),
  go: job({
    id: 4,
    session: "9a7c03de",
    cwd: "/home/user/code/ingest-svc",
    cores: 0.9,
    ramMb: 614,
    command: "go vet ./...",
    jobClass: "light",
    process: "go",
    pid: 14772,
    profile: { cores: 1, ramMb: 1024 },
    elapsed: 20,
    estimate: 60,
  }),
  npm: job({
    id: 5,
    session: "77a0e5d2",
    cwd: "/home/user/code/customer-portal-monorepo",
    cores: 0.4,
    ramMb: 1126,
    command: "npm run test:e2e",
    profile: { cores: 2, ramMb: 4096 },
    waited: 42,
    waitLeft: 78,
  }),
  npmB: job({
    id: 6,
    session: "77a0e5d2",
    cwd: "/home/user/code/customer-portal-monorepo",
    cores: 0.4,
    ramMb: 1126,
    command: "npm run test:e2e",
    profile: { cores: 2, ramMb: 4096 },
    waited: 72,
  }),
  npmN: job({
    id: 7,
    session: "77a0e5d2",
    cwd: "/home/user/code/customer-portal-monorepo",
    cores: 0.4,
    ramMb: 1126,
    command: "npm run test:e2e",
    profile: { cores: 2, ramMb: 4096 },
    waited: 196,
    waitLeft: 17,
  }),
  ctest: job({
    id: 8,
    session: "3f2a9c1e",
    cwd: "/home/user/code/allot",
    cores: 7.8,
    ramMb: 2970,
    command: "cargo test --workspace",
    jobClass: null,
    profile: { cores: 8, ramMb: 4096 },
    waited: 18,
  }),
  bench: job({
    id: 9,
    session: "c41f9a07",
    cwd: "/home/user/code/parser-perf",
    cores: 0.2,
    ramMb: 410,
    command: "cargo bench --bench parser",
    process: "cargo",
    pid: 30412,
    profile: { cores: 16, ramMb: 8192 },
    elapsed: 48,
    estimate: 120,
    waited: 30,
  }),
};

// Executables of apps the GNOME test session has, for the icons scenario; go is a plain
// command-line tool, so it keeps its initials.
const EXES = {
  cargo: "/usr/bin/gnome-control-center",
  py: "/usr/bin/byobu",
  vite: "/usr/bin/vim",
  go: "/usr/local/go/bin/go",
};

/**
 * @param {string} name
 * @returns {number}
 */
export function leaseId(name) {
  return JOBS[name]?.id ?? 0;
}

/**
 * @param {string} name
 * @returns {string}
 */
export function sessionOf(name) {
  return fullSession(JOBS[name]?.session ?? "");
}

/**
 * The service status and state of a scenario in `SCENARIO_NAMES`, as JSON before decoding.
 *
 * @param {string} name
 * @returns {{ status: ServiceStatus, json: Record<string, unknown> }}
 */
export function scenarioJson(name) {
  switch (name) {
    case "empty":
      return running(stateJson({ recent: true }));
    case "one":
    case "c-active":
    case "c-confirm":
    case "c-killed":
    case REFUSED_SCENARIO:
      return running(stateJson({ run: ["cargo"] }));
    case "several":
    case "kill-confirm":
    case "killed":
      return running(stateJson({ run: ["cargo", "py"], wait: ["npm"], idle: true }));
    case "c-removed":
      return running(stateJson({ wait: ["npm"] }));
    case "bench-drain":
      return running(stateJson({ run: ["cargo"], wait: ["npmB"], bench: "drain" }));
    case "bench-quiet":
    case "bench-run":
    case "bench-noisy":
    case "bench-forced":
      return running(stateJson({ wait: ["npmB"], bench: name.slice("bench-".length) }));
    case "bench-done":
      return running(stateJson({ wait: ["npmN"], bench: "done" }));
    case "down":
      return { status: "stopped", json: stateJson({ run: ["cargo"] }) };
    case "max":
    case "icons":
      return running(
        stateJson({
          run: ["cargo", "py", "vite", "go"],
          wait: ["npm", "ctest"],
          idle: true,
          exes: name === "icons",
        }),
      );
    case "c-over":
      return running(stateJson({ run: ["py"] }));
    case "c-queued":
      return running(stateJson({ wait: ["npm", "ctest"] }));
    case "c-bench":
      return running(stateJson({ bench: "quiet" }));
    default:
      throw new Error(`unknown scenario ${name}`);
  }
}

/**
 * @param {string} name
 * @returns {{ status: ServiceStatus, state: UiState }}
 */
export function scenario(name) {
  const { status, json } = scenarioJson(name);
  return { status, state: decodeUiState(json) };
}

/**
 * The Kill and Remove states a scenario starts in (the prototype's `init`): Kill? with 3 s left,
 * or Stopped or Removed at the scenario's moment. Empty for the others.
 *
 * @param {string} name
 * @returns {Map<number, StopView>}
 */
export function stops(name) {
  switch (name) {
    case "kill-confirm":
    case "c-confirm":
      return new Map([[leaseId("cargo"), stopView("kill", "confirming", 3, null)]]);
    case "killed":
    case "c-killed":
      return new Map([[leaseId("cargo"), stopView("kill", "ended", 0, NOW_MS)]]);
    case "c-removed":
      return new Map([[leaseId("npm"), stopView("remove", "ended", 0, NOW_MS)]]);
    default:
      return new Map();
  }
}

/**
 * @param {Record<string, unknown>} json
 * @returns {{ status: ServiceStatus, json: Record<string, unknown> }}
 */
function running(json) {
  return { status: "running", json };
}

/**
 * @param {import("../allot@mehran.tech/lib/job-stops.js").StopKind} kind
 * @param {import("../allot@mehran.tech/lib/job-stops.js").StopPhase} phase
 * @param {number} secondsLeft
 * @param {number | null} endedAtMs
 * @returns {StopView}
 */
function stopView(kind, phase, secondsLeft, endedAtMs) {
  return { kind, phase, secondsLeft, message: "", endedAtMs };
}

/**
 * @typedef {object} StateOptions
 * @property {string[]} [run] Active jobs, in grant order.
 * @property {string[]} [wait] Queued jobs, in queue order after a queued benchmark.
 * @property {string} [bench] `drain`, `quiet`, `noisy` (waiting for quiet past the limits),
 *   `run`, `forced` (running after the longest quiet wait) or `done` (finished 10 s ago).
 * @property {boolean} [idle] Adds the idle session `e09c7b3a`.
 * @property {boolean} [recent] Adds the two Recently finished runs of the `empty` scenario.
 * @property {boolean} [exes] Gives the processes executables of installed apps.
 */

/**
 * @param {StateOptions} options
 * @returns {Record<string, unknown>}
 */
export function stateJson(options) {
  const { run = [], wait = [], bench = null, idle = false, recent = false } = options;
  const exes = options.exes ?? false;
  const activeJobs = run.map((name) => active(name, exes, false));
  /** @type {object[]} */
  const queuedJobs = [];
  const used = [...run, ...wait];
  const benchRuns = bench === "run" || bench === "forced";
  if (bench !== null) {
    used.push("bench");
  }
  if (benchRuns) {
    activeJobs.push({ ...active("bench", exes, true), quiet_at_start: bench === "run" });
  } else if (bench === "drain" || bench === "quiet" || bench === "noisy") {
    const { progress, waitLeft } = benchmarkWait(bench, run.length);
    queuedJobs.push(queued(only("bench"), 1, true, progress, waitLeft));
  }
  for (const name of wait) {
    queuedJobs.push(queued(only(name), queuedJobs.length + 1, false, null, null));
  }

  // A benchmark running alone has the machine's load to itself, as on the board.
  const seen = new Set();
  const sessions = [];
  for (const each of used.map(only)) {
    if (seen.has(each.session)) {
      continue;
    }
    seen.add(each.session);
    const isBench = benchRuns && each.id === leaseId("bench");
    const cores = isBench ? 15.6 : each.cores;
    sessions.push(session(fullSession(each.session), each.cwd, cores, isBench ? 3174 : each.ramMb));
  }
  if (idle) {
    sessions.push(session(IDLE_SESSION, "/home/user/code/notes", 0.1, 307));
  }

  /** @type {object[]} */
  let recentRuns = [];
  if (bench === "done") {
    recentRuns.push(finishedRunJson(true));
  }
  if (recent) {
    recentRuns = [runJson("cargo", 240, 292), runJson("npm", 660, 130)];
  }
  return {
    generated_at_ms: NOW_MS,
    service: { version: "0.3.0", pid: 4242, uptime_secs: 600, config_error: null },
    first_session_at_ms: FIRST_SESSION_MS,
    sessions,
    active: activeJobs,
    queue: queuedJobs,
    meters: METERS,
    recent: recentRuns,
    notices: [],
    ended: [],
  };
}

/**
 * @param {Record<string, unknown>} json
 * @returns {UiState}
 */
export function decodeState(json) {
  return decodeUiState(json);
}

/**
 * The benchmark of `bench-done`: it ran 2:04 and ended 10 s before the scenario's moment, with
 * background noise avg 1.8%, peak 6.2% CPU.
 *
 * @param {boolean} quietAtStart
 * @returns {RecentRun}
 */
export function finishedBenchmark(quietAtStart = true) {
  const state = decodeUiState({ ...stateJson({}), recent: [finishedRunJson(quietAtStart)] });
  const [run] = state.recent;
  if (run === undefined) {
    throw new Error("the finished run did not decode");
  }
  return run;
}

/**
 * `state` without the lease, as allotd lists it once the lease has ended, plus how it ended.
 *
 * @param {UiState} state
 * @param {number} lease
 * @param {import("../allot@mehran.tech/contract.g.mjs").EndCause | null} [cause]
 * @returns {UiState}
 */
export function without(state, lease, cause = null) {
  const job = [...state.active, ...state.queue].find((each) => each.lease.id === lease);
  const ended =
    cause === null || job === undefined
      ? []
      : [
          {
            cause,
            command: job.lease.label,
            ended_at_ms: state.generated_at_ms,
            granted_at_ms: job.lease.granted_at_ms,
            lease_id: lease,
            reason: cause === "expired" ? "the holder is gone" : "",
            session: job.lease.holder.session,
          },
        ];
  return {
    ...state,
    active: state.active.filter((each) => each.lease.id !== lease),
    queue: state.queue.filter((each) => each.lease.id !== lease),
    ended,
  };
}

const METERS = {
  headroom_pct: 15.0,
  cpu: { logical_cores: 12, usage_pct: 6.0, history_pct: [6.0], reserved_cores: 0.0 },
  ram: { total_mb: 32768, used_mb: 14438, reserved_mb: 0 },
  gpus: [],
  drives: [],
};

/**
 * @param {string} name
 * @returns {Job}
 */
function only(name) {
  const found = JOBS[name];
  if (found === undefined) {
    throw new Error(`unknown job ${name}`);
  }
  return found;
}

/**
 * @param {string} shortId
 * @returns {string}
 */
function fullSession(shortId) {
  return `${shortId}-1111-4000-8000-000000000000`;
}

/**
 * @param {string} id
 * @param {string} cwd
 * @param {number} cores
 * @param {number} ramMb
 * @returns {object}
 */
function session(id, cwd, cores, ramMb) {
  return { session: id, cwd, usage: { cores, ram_mb: ramMb, gpu_pct: 0.0, vram_mb: 0 } };
}

/**
 * @param {string} name
 * @param {boolean} exes
 * @param {boolean} benchmark
 * @returns {Record<string, unknown>}
 */
function active(name, exes, benchmark) {
  const each = only(name);
  const exe = exes ? (EXES[/** @type {keyof typeof EXES} */ (name)] ?? null) : null;
  const process =
    each.process === null ? null : { bound: true, exe, name: each.process, pid: each.pid };
  return { lease: lease(each, true, benchmark), process, quiet_at_start: null };
}

/**
 * Draining behind the running jobs, 6 of 10 s of quiet at 3% CPU, or 14% CPU against a 5%
 * limit with 42 s left before allotd grants it anyway.
 *
 * @param {string} bench
 * @param {number} runningCount
 * @returns {{ progress: object, waitLeft: number | null }}
 */
function benchmarkWait(bench, runningCount) {
  switch (bench) {
    case "quiet":
      return {
        progress: {
          phase: "measuring",
          cpu_avg_pct: 3.0,
          gpu_avg_pct: 0.0,
          measured_secs: 6,
          needed_secs: 10,
        },
        waitLeft: 4,
      };
    case "noisy":
      return {
        progress: {
          phase: "noisy",
          cpu_avg_pct: 14.0,
          cpu_limit_pct: 5.0,
          gpu_avg_pct: 0.0,
          gpu_limit_pct: 10.0,
          grant_anyway_in_secs: 42,
        },
        waitLeft: 42,
      };
    default:
      return { progress: { phase: "draining", active_leases: runningCount }, waitLeft: null };
  }
}

/**
 * @param {Job} each
 * @param {number} position
 * @param {boolean} benchmark
 * @param {object | null} progress
 * @param {number | null} waitLeft
 * @returns {object}
 */
function queued(each, position, benchmark, progress, waitLeft) {
  return {
    lease: lease(each, false, benchmark),
    position,
    verdict: {
      benchmark: progress,
      fits: false,
      reasons: ["not enough free cores"],
      wait_estimate_secs: waitLeft ?? each.waitLeft,
    },
  };
}

/**
 * @param {Job} each
 * @param {boolean} isActive
 * @param {boolean} benchmark
 * @returns {object}
 */
function lease(each, isActive, benchmark) {
  const grantedAtMs = NOW_MS - each.elapsed * 1000;
  return {
    id: each.id,
    holder: {
      session: fullSession(each.session),
      pid: 1000 + each.id,
      pid_start_time: 1_792_824_000,
    },
    kind: benchmark ? "benchmark_exclusive" : "normal",
    state: isActive ? "active" : "queued",
    class: each.jobClass,
    profile: profile(each.profile, each.estimate),
    label: each.command,
    cwd: each.cwd,
    requested_at_ms: isActive ? grantedAtMs - 5000 : NOW_MS - each.waited * 1000,
    granted_at_ms: isActive ? grantedAtMs : null,
    expires_at_ms: NOW_MS + 30_000,
  };
}

/**
 * @param {boolean} quietAtStart
 * @returns {object}
 */
function finishedRunJson(quietAtStart) {
  const each = only("bench");
  const endedAtMs = NOW_MS - 10_000;
  return {
    command: each.command,
    cwd: each.cwd,
    declared: profile(each.profile, 300),
    end_cause: "released",
    ended_at_ms: endedAtMs,
    lease_id: each.id,
    noise: {
      avg_cpu_pct: 1.8,
      avg_gpu_pct: 0.0,
      peak_cpu_pct: 6.2,
      peak_gpu_pct: 0.0,
      quiet_at_start: quietAtStart,
    },
    session: fullSession(each.session),
    started_at_ms: endedAtMs - 124_000,
  };
}

/**
 * @param {string} name
 * @param {number} endedAgoSecs
 * @param {number} ranSecs
 * @returns {object}
 */
function runJson(name, endedAgoSecs, ranSecs) {
  const each = only(name);
  const endedAtMs = NOW_MS - endedAgoSecs * 1000;
  return {
    command: each.command,
    cwd: each.cwd,
    declared: profile(each.profile, 300),
    end_cause: "released",
    ended_at_ms: endedAtMs,
    lease_id: each.id + 100,
    session: fullSession(each.session),
    started_at_ms: endedAtMs - ranSecs * 1000,
  };
}

/**
 * @param {Profile} value
 * @param {number} estSecs
 * @returns {object}
 */
function profile(value, estSecs) {
  return {
    cores: value.cores,
    ram_mb: value.ramMb,
    gpu_pct: value.gpuPct ?? 0,
    vram_mb: value.vramMb ?? 0,
    est_secs: estSecs,
  };
}
