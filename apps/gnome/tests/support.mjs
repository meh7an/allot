import { readFileSync } from "node:fs";

import { decodeUiState } from "../allot@mehran.tech/contract.g.mjs";

/** @import { UiState } from "../allot@mehran.tech/contract.g.mjs" */
/** @import { HttpRequest, HttpResponse } from "../allot@mehran.tech/lib/client.js" */

export const EXTENSION_DIR = new URL("../allot@mehran.tech/", import.meta.url);
export const REPO_DIR = new URL("../../../", import.meta.url);
const FIXTURE_DIR = new URL("contract/fixtures/", REPO_DIR);

/**
 * @param {string} file A file name under `contract/fixtures`.
 * @returns {string}
 */
export function fixtureText(file) {
  return readFileSync(new URL(file, FIXTURE_DIR), "utf8");
}

/**
 * @param {string} name The `ui-state-<name>.json` fixture.
 * @returns {UiState}
 */
export function fixtureState(name) {
  return decodeUiState(JSON.parse(fixtureText(`ui-state-${name}.json`)));
}

/**
 * Builds UI states from a few jobs: the first-run fixture with sessions, active and queue
 * replaced, decoded like a real poll.
 *
 * @param {{ sessions?: object[], active?: object[], queue?: object[] }} jobs
 * @returns {UiState}
 */
export function buildState(jobs) {
  const base = JSON.parse(fixtureText("ui-state-first-run.json"));
  base.sessions = jobs.sessions ?? [];
  base.active = jobs.active ?? [];
  base.queue = jobs.queue ?? [];
  return decodeUiState(base);
}

/**
 * @param {string} id
 * @param {{ cwd?: string | null, cores?: number, ramMb?: number }} [options]
 * @returns {object}
 */
export function session(id, options = {}) {
  return {
    session: id,
    cwd: options.cwd === undefined ? "/home/user/work/app" : options.cwd,
    usage: { cores: options.cores ?? 1.5, ram_mb: options.ramMb ?? 2048, gpu_pct: 0, vram_mb: 0 },
  };
}

/**
 * @param {number} id
 * @param {string} holder
 * @param {{ label?: string, benchmark?: boolean, cwd?: string }} [options]
 * @returns {object}
 */
export function active(id, holder, options = {}) {
  return {
    lease: lease(id, holder, options.label ?? "cargo build", options, "active"),
    process: null,
    quiet_at_start: null,
  };
}

/**
 * @param {number} id
 * @param {string} holder
 * @param {{ label?: string, benchmark?: boolean, phase?: string, cwd?: string }} [options]
 *   `phase` is the benchmark verdict's phase: `draining`, `measuring` or `noisy`.
 * @returns {object}
 */
export function queued(id, holder, options = {}) {
  return {
    lease: lease(id, holder, options.label ?? "cargo test", options, "queued"),
    position: 1,
    verdict: {
      fits: false,
      reasons: [],
      wait_estimate_secs: null,
      benchmark: benchmarkVerdict(options.phase),
    },
  };
}

/**
 * @param {string | undefined} phase
 * @returns {object | null}
 */
function benchmarkVerdict(phase) {
  switch (phase) {
    case "draining":
      return { phase, active_leases: 1 };
    case "measuring":
      return { phase, measured_secs: 4, needed_secs: 10, cpu_avg_pct: 3, gpu_avg_pct: 0 };
    case "noisy":
      return {
        phase,
        cpu_avg_pct: 30,
        cpu_limit_pct: 10,
        gpu_avg_pct: 0,
        gpu_limit_pct: 10,
        grant_anyway_in_secs: 50,
      };
    default:
      return null;
  }
}

/**
 * @param {number} id
 * @param {string} holder
 * @param {string} label
 * @param {{ benchmark?: boolean, cwd?: string }} options
 * @param {"active" | "queued"} state
 * @returns {object}
 */
function lease(id, holder, label, options, state) {
  return {
    id,
    holder: { session: holder, pid: 100, pid_start_time: 1 },
    kind: options.benchmark ? "benchmark_exclusive" : "normal",
    state,
    class: "cpu_heavy",
    profile: { cores: 4, ram_mb: 2048, gpu_pct: 0, vram_mb: 0, est_secs: 60 },
    label,
    cwd: options.cwd ?? "/home/user/work/app",
    requested_at_ms: 1_792_824_700_000,
    granted_at_ms: state === "active" ? 1_792_824_710_000 : null,
    ended_at_ms: null,
    expires_at_ms: 1_792_824_765_000,
  };
}

/** Answers each request from a handler and records what was sent. */
export class FakeTransport {
  /** @type {HttpRequest[]} */
  requests = [];

  #handler;

  /** @param {(request: HttpRequest) => HttpResponse | Promise<HttpResponse>} handler */
  constructor(handler) {
    this.#handler = handler;
  }

  /**
   * @param {HttpRequest} request
   * @returns {Promise<HttpResponse>}
   */
  send = async (request) => {
    this.requests.push(request);
    return this.#handler(request);
  };
}

/** Timers that fire only when a test says so. */
export class FakeTimers {
  /** @type {Map<number, { callback: () => void, ms: number }>} */
  pending = new Map();

  #next = 1;

  /**
   * @param {() => void} callback
   * @param {number} ms
   * @returns {number}
   */
  setTimeout = (callback, ms) => {
    const id = this.#next++;
    this.pending.set(id, { callback, ms });
    return id;
  };

  /** @param {unknown} id */
  clearTimeout = (id) => {
    this.pending.delete(/** @type {number} */ (id));
  };

  /** @returns {number[]} The delays of the pending timers. */
  get delays() {
    return [...this.pending.values()].map((timer) => timer.ms);
  }

  fireAll() {
    const timers = [...this.pending.values()];
    this.pending.clear();
    for (const timer of timers) {
      timer.callback();
    }
  }
}

/**
 * Lets queued promise callbacks run.
 *
 * @returns {Promise<void>}
 */
export function settle() {
  return new Promise((resolve) => setImmediate(resolve));
}
