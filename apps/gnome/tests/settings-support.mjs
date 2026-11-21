import assert from "node:assert/strict";

import { decodeSettingsState } from "../allot@mehran.tech/contract.g.mjs";
import { SettingsSession } from "../allot@mehran.tech/lib/settings-session.js";
import { FakeTimers, fixtureText, settle } from "./support.mjs";

/**
 * @import { Change, SettingsState, Shell } from "../allot@mehran.tech/contract.g.mjs"
 * @import { SettingsFailure, SettingsReply } from "../allot@mehran.tech/lib/results.js"
 * @import { ValueModel } from "../allot@mehran.tech/lib/setting-values.js"
 */

// What the settings tests share: the settings-state fixture, a fake settings client, and a
// session loaded from it.

/**
 * The settings-state fixture, changed by `edit` before it is decoded.
 *
 * @param {(json: any) => void} [edit]
 * @returns {SettingsState}
 */
export function fixtureState(edit) {
  const json = JSON.parse(fixtureText("settings-state.json"));
  edit?.(json);
  return decodeSettingsState(json);
}

/**
 * Replaces the value of `key` in a settings-state object.
 *
 * @param {any} json
 * @param {string} key
 * @param {unknown} value
 */
export function setValue(json, key, value) {
  const entry = json.values.find((/** @type {{ key: string }} */ candidate) =>
    candidate.key === key
  );
  entry.value = value;
}

/**
 * Adds a preset of the user's own, `render`, to a settings-state object.
 *
 * @param {any} json
 */
export function withRender(json) {
  const presets = json.values.find((/** @type {{ key: string }} */ value) =>
    value.key === "presets"
  );
  presets.value.push({
    name: "render",
    profile: { cores: 6, ram_mb: 8192, gpu_pct: 40, vram_mb: 4096, est_secs: 900 },
  });
}

/**
 * @param {SettingsFailure["kind"]} kind
 * @param {Partial<SettingsFailure>} [fields]
 * @returns {{ ok: false, failure: SettingsFailure }}
 */
export function failure(kind, fields = {}) {
  return {
    ok: false,
    failure: {
      kind,
      message: fields.message ?? kind,
      problems: fields.problems ?? [],
      revision: fields.revision ?? null,
    },
  };
}

/** A settings client whose answers each test queues; it records every request. */
export class FakeSettings {
  /** @type {SettingsReply<SettingsState>[]} */
  reads = [];
  /** @type {SettingsReply<SettingsState>[]} */
  writes = [];
  /** @type {{ revision: string, changes: Change[] }[]} */
  patches = [];
  /** @type {Change[][]} */
  previews = [];
  /** @type {{ line: string, shell: string, changes: Change[] }[]} */
  classified = [];
  state = fixtureState();

  settings = async () => this.reads.shift() ?? { ok: true, value: this.state };

  /** @param {{ revision: string, changes: Change[] }} patch */
  changeSettings = async (patch) => {
    this.patches.push(patch);
    return this.writes.shift() ?? { ok: true, value: this.state };
  };

  /** @param {Change[]} changes */
  previewSettings = async (changes) => {
    this.previews.push(changes);
    return failure("unavailable");
  };

  /** @param {{ line: string, shell: Shell, changes: Change[] }} request */
  classify = async (request) => {
    this.classified.push(request);
    return failure("unavailable");
  };
}

/**
 * A session loaded from the settings-state fixture.
 *
 * @param {object} [options]
 * @param {"windows" | "macos" | "linux"} [options.platform]
 * @param {string | null} [options.stopRecord]
 * @param {SettingsState} [options.state]
 */
export async function loaded(options = {}) {
  const client = new FakeSettings();
  if (options.state !== undefined) {
    client.state = options.state;
  }
  const timers = new FakeTimers();
  const session = new SettingsSession(client, timers, {
    platform: options.platform ?? "linux",
    readStopRecord: async () => options.stopRecord ?? null,
  });
  await session.refresh();
  return { session, client, timers };
}

/**
 * Lets a save in flight finish, fires the burst timer, and lets that save finish.
 *
 * @param {FakeTimers} timers
 */
export async function fire(timers) {
  await settleAll();
  timers.fireAll();
  await settleAll();
}

export async function settleAll() {
  for (let round = 0; round < 5; round += 1) {
    await settle();
  }
}

/**
 * @template T
 * @param {ValueModel<unknown> | undefined} model
 * @param {new (...args: never[]) => T} type
 * @returns {T}
 */
export function as(model, type) {
  assert.ok(model instanceof type, `${model?.key} is a ${type.name}`);
  return model;
}

/** Formats numbers as en-US does, grouped with at most two decimals. */
export const english = (/** @type {number} */ value) =>
  new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 }).format(value);
