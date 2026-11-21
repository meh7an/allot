import { decodeStopRecord } from "../contract.g.mjs";
import { CancelledError } from "./client.js";
import { valueModel } from "./setting-values.js";

/**
 * @import {
 *   Change,
 *   CommandTest,
 *   FitPreview,
 *   Platform,
 *   SettingsGroupSpec,
 *   SettingsPageSpec,
 *   SettingsState,
 *   Shell,
 *   StopRecord,
 * } from "../contract.g.mjs"
 * @import { SettingsFailure, SettingsReply } from "./client.js"
 * @import { Timers } from "./poller.js"
 * @import { ValueHost, ValueModel } from "./setting-values.js"
 */

/** Single values save this long after the last change, as one `PATCH`. */
export const SAVE_DELAY_MS = 600;

/** While the window is visible, values are fetched again this often. */
export const REFRESH_INTERVAL_MS = 5000;

/**
 * - `loading`: no answer yet.
 * - `ready`: allotd answered; the values can be changed unless `fileError` is set.
 * - `stopped`: allotd does not answer and the stop record says the user stopped it.
 * - `down`: allotd does not answer.
 * - `token`: allotd refused this panel's key.
 * - `failed`: allotd answered something unusable; `detail` says what.
 *
 * Every state but `ready` keeps the last values read-only.
 *
 * @typedef {"loading" | "ready" | "stopped" | "down" | "token" | "failed"} SessionStatus
 */

/**
 * The save status line: "All changes saved", "Saving…" or "{n} changes not saved".
 *
 * @typedef {{ kind: "saved" } | { kind: "saving" } | { kind: "unsaved", count: number }}
 *   SaveStatus
 */

/**
 * @typedef {object} GroupModel
 * @property {SettingsGroupSpec} spec
 * @property {ValueModel<unknown>[]} settings
 *   The group's settings this platform shows.
 */

/**
 * @typedef {object} PageModel
 * @property {SettingsPageSpec} spec
 * @property {GroupModel[]} groups
 */

/**
 * A change allotd did not write because `config.toml` changed by hand since it was read: the
 * setting, which shows the file's value now, and the changes that would write the user's.
 *
 * @typedef {object} ConflictEdit
 * @property {ValueModel<unknown>} setting
 * @property {Change[]} changes
 */

/**
 * The edits a hand edit got in the way of; Apply my change sends them again.
 *
 * @typedef {object} Conflict
 * @property {ConflictEdit[]} edits
 */

/**
 * The settings routes the session uses: a `DaemonClient`, or a fake in tests.
 *
 * @typedef {object} SettingsClient
 * @property {() => Promise<SettingsReply<SettingsState>>} settings
 * @property {(patch: { revision: string, changes: Change[] }) =>
 *   Promise<SettingsReply<SettingsState>>} changeSettings
 * @property {(changes: Change[]) => Promise<SettingsReply<FitPreview>>} previewSettings
 * @property {(request: { line: string, shell: Shell, changes: Change[] }) =>
 *   Promise<SettingsReply<CommandTest>>} classify
 */

/**
 * @typedef {object} Pending
 * @property {ValueModel<unknown>} model
 * @property {Change[]} changes
 * @property {number} generation
 */

/**
 * The Settings window's allotd settings: one view model per setting, fetched from
 * `GET /settings` and saved through `PATCH /settings` with the file's revision. Single values
 * save as one burst 600 ms after the last change; tables, rules and chips save at once. The
 * Panel page's own settings are not here: they stay in each panel's store.
 *
 * @implements {ValueHost}
 */
export class SettingsSession {
  /** @type {SessionStatus} */
  status = "loading";
  /** @type {string | null} */
  detail = null;
  /** @type {StopRecord | null} */
  stopRecord = null;
  /** @type {string | null} */
  fileError = null;
  /** @type {string[]} */
  pendingRestart = [];
  /** @type {number | null} */
  listeningPort = null;
  /** @type {PageModel[]} */
  pages = [];
  /** @type {Conflict | null} */
  conflict = null;
  /**
   * Why the last save was not written though allotd answered, such as a held file; the
   * changes stay unsaved until the next save.
   *
   * @type {string | null}
   */
  writeProblem = null;
  /** @type {(() => void) | null} */
  onChange = null;

  #client;
  #timers;
  #platform;
  #readStopRecord;
  /** @type {string | null} */
  #revision = null;
  /** @type {Map<string, ValueModel<unknown>>} */
  #models = new Map();
  /** @type {Map<string, Pending>} */
  #pending = new Map();
  /** @type {unknown} */
  #saveTimer = null;
  /** @type {unknown} */
  #refreshTimer = null;
  #saving = false;
  /**
   * Whether the last save failed in a way waiting does not fix. The burst timer then stays off
   * until Try again or the next edit, so allotd is not asked every 600 ms.
   */
  #stalled = false;
  /**
   * Counts saves that started or finished, so a refresh reply sent before one is dropped: it
   * would bring back the revision and values from before the write.
   */
  #writes = 0;
  #visible = false;
  #disposed = false;

  /**
   * @param {SettingsClient} client
   * @param {Timers} timers
   * @param {object} options
   * @param {Platform} options.platform Settings for other platforms are left out.
   * @param {() => Promise<string | null>} options.readStopRecord
   *   The text of `~/.allot/stopped.json`, or `null` when there is none.
   */
  constructor(client, timers, options) {
    this.#client = client;
    this.#timers = timers;
    this.#platform = options.platform;
    this.#readStopRecord = options.readStopRecord;
  }

  /** Whether allotd's settings cannot be changed now. */
  get readOnly() {
    return this.status !== "ready" || this.fileError !== null;
  }

  /** @returns {SaveStatus} */
  get saveStatus() {
    if (this.#saving) {
      return { kind: "saving" };
    }
    let count = this.#pending.size;
    for (const model of this.#models.values()) {
      if (model.issue !== null && model.dirty && !this.#pending.has(model.key)) {
        count += 1;
      }
    }
    return count === 0 ? { kind: "saved" } : { kind: "unsaved", count };
  }

  /**
   * @param {string} key
   * @returns {ValueModel<unknown> | undefined}
   */
  setting(key) {
    return this.#models.get(key);
  }

  /**
   * The settings on a page whose saved value differs from the default, for Reset page.
   *
   * @param {string} pageId
   * @returns {ValueModel<unknown>[]}
   */
  changedOn(pageId) {
    const page = this.pages.find((candidate) => candidate.spec.id === pageId);
    return page === undefined ? [] : changed([page]);
  }

  /** Every changed setting on every page, for Reset all. */
  changedSettings() {
    return changed(this.pages);
  }

  /**
   * Fetches the values again: on show, on focus and every 5 s while visible. Changed values
   * update in place; edits not yet saved stay.
   *
   * @returns {Promise<void>} Rejects only with `CancelledError`.
   */
  async refresh() {
    const writes = this.#writes;
    const reply = await this.#client.settings();
    if (this.#disposed || writes !== this.#writes) {
      return;
    }
    if (reply.ok) {
      this.#take(reply.value);
      if (this.#pending.size > 0 && !this.readOnly && !this.#stalled) {
        this.#schedule(0);
      }
    } else {
      await this.#unanswered(reply.failure);
    }
    this.#notify();
  }

  /**
   * Starts or stops fetching every 5 s. Hiding the window sends a pending burst first.
   *
   * @param {boolean} visible
   */
  setVisible(visible) {
    this.#visible = visible;
    this.#clearRefresh();
    if (visible) {
      this.#tick();
    } else if (this.#pending.size > 0) {
      void this.flush();
    }
  }

  /**
   * Sends the pending changes now instead of after the burst.
   *
   * @returns {Promise<void>}
   */
  async flush() {
    this.#clearSave();
    if (this.#pending.size === 0 && !this.#saving) {
      // Try again with nothing left to save only dismisses a problem that undid the edits.
      this.writeProblem = null;
    }
    if (this.#saving || this.#pending.size === 0 || this.readOnly || this.#revision === null) {
      this.#notify();
      return;
    }
    const batch = [...this.#pending.values()];
    this.#pending.clear();
    this.#saving = true;
    this.#stalled = false;
    this.#writes += 1;
    this.#notify();
    /** @type {Change[]} */
    const changes = [];
    for (const entry of batch) {
      changes.push(...entry.changes);
    }
    let reply;
    try {
      reply = await this.#client.changeSettings({ revision: this.#revision, changes });
    } catch (error) {
      if (error instanceof CancelledError) {
        return;
      }
      throw error;
    } finally {
      this.#saving = false;
      this.#writes += 1;
    }
    if (this.#disposed) {
      return;
    }
    let sendSoon = true;
    if (reply.ok) {
      this.writeProblem = null;
      for (const entry of batch) {
        entry.model.markSaved(entry.generation);
      }
      this.#take(reply.value);
    } else {
      await this.#refused(batch, reply.failure);
      // Only a held file frees up on its own within moments. Changes refused because allotd
      // did not answer go out again from the refresh that finds it back; any other refusal
      // waits for Try again or the next edit.
      sendSoon = reply.failure.kind === "held";
    }
    if (this.#pending.size > 0 && this.#saveTimer === null && sendSoon) {
      this.#schedule(SAVE_DELAY_MS);
    }
    this.#notify();
  }

  /** Sends the changes a hand edit got in the way of again, over the file's values. */
  applyMine() {
    const conflict = this.conflict;
    if (conflict === null) {
      return;
    }
    this.conflict = null;
    for (const { setting, changes } of conflict.edits) {
      this.#pending.set(setting.key, { model: setting, changes, generation: setting.generation });
    }
    void this.flush();
  }

  /** Drops the changes a hand edit got in the way of; the file's values stay. */
  keepFileValues() {
    this.conflict = null;
    this.#notify();
  }

  /**
   * Resets every changed setting on a page, or on every page, in one write. The window asks
   * first.
   *
   * @param {string | null} pageId `null` for Reset all.
   */
  resetChanged(pageId) {
    const settings = pageId === null ? this.changedSettings() : this.changedOn(pageId);
    for (const model of settings) {
      model.reset();
    }
    void this.flush();
  }

  /**
   * What fits with the shown values, unsaved edits included, and `changes`, such as a row edited
   * in a dialog.
   *
   * @param {Change[]} [changes]
   * @returns {Promise<SettingsReply<FitPreview>>} Rejects only with `CancelledError`.
   */
  preview(changes = []) {
    return this.#client.previewSettings([...this.#unsaved(), ...changes]);
  }

  /**
   * How the hook treats `line`, under the shown values and `changes`, such as a rule list
   * edited in a dialog.
   *
   * @param {string} line
   * @param {Shell} shell
   * @param {Change[]} [changes]
   * @returns {Promise<SettingsReply<CommandTest>>} Rejects only with `CancelledError`.
   */
  classify(line, shell, changes = []) {
    return this.#client.classify({ line, shell, changes: [...this.#unsaved(), ...changes] });
  }

  /** Stops the timers; answers that arrive later are dropped. */
  dispose() {
    this.#disposed = true;
    this.#clearSave();
    this.#clearRefresh();
  }

  /**
   * @param {string} key
   * @returns {unknown}
   */
  valueOf(key) {
    return this.#models.get(key)?.value;
  }

  /**
   * @param {string} key
   * @returns {string}
   */
  labelOf(key) {
    return this.#models.get(key)?.label ?? key;
  }

  /**
   * @param {ValueModel<unknown>} model
   * @param {Change[]} changes
   * @param {boolean} now
   */
  edited(model, changes, now) {
    this.#stalled = false;
    if (changes.length === 0) {
      this.#pending.delete(model.key);
    } else {
      this.#pending.set(model.key, { model, changes, generation: model.generation });
    }
    if (now) {
      void this.flush();
      return;
    }
    if (changes.length > 0) {
      this.#schedule(SAVE_DELAY_MS);
    }
    this.#notify();
  }

  /** @param {SettingsState} state */
  #take(state) {
    this.status = "ready";
    this.detail = null;
    this.stopRecord = null;
    this.#revision = state.revision;
    this.fileError = state.file_error;
    this.pendingRestart = state.pending_restart;
    this.listeningPort = state.listening_port;
    /** @type {Map<string, unknown>} */
    const values = new Map();
    for (const entry of state.values) {
      values.set(entry.key, entry.value);
    }
    if (this.#models.size === 0) {
      this.pages = this.#build(state, values);
      return;
    }
    for (const model of this.#models.values()) {
      if (values.has(model.key)) {
        model.load(values.get(model.key));
      }
    }
  }

  /**
   * @param {SettingsState} state
   * @param {Map<string, unknown>} values
   * @returns {PageModel[]}
   */
  #build(state, values) {
    /** @type {PageModel[]} */
    const pages = [];
    for (const page of state.spec.pages) {
      /** @type {GroupModel[]} */
      const groups = [];
      for (const group of page.groups) {
        const settings = group.settings
          .filter((spec) => spec.platforms.length === 0 || spec.platforms.includes(this.#platform))
          .filter((spec) => values.has(spec.key))
          .map((spec) => valueModel(spec, this, values.get(spec.key)));
        for (const model of settings) {
          this.#models.set(model.key, model);
        }
        if (settings.length > 0) {
          groups.push({ spec: group, settings });
        }
      }
      pages.push({ spec: page, groups });
    }
    return pages;
  }

  /** @param {SettingsFailure} failure */
  async #unanswered(failure) {
    this.detail = failure.message;
    switch (failure.kind) {
      case "unreachable":
        this.stopRecord = parseStopRecord(await this.#readStopRecord());
        this.status = this.stopRecord === null ? "down" : "stopped";
        break;
      case "token":
        this.status = "token";
        break;
      default:
        this.status = "failed";
    }
  }

  /**
   * @param {Pending[]} batch
   * @param {SettingsFailure} failure
   */
  async #refused(batch, failure) {
    switch (failure.kind) {
      case "conflict":
        this.conflict = {
          edits: batch.map((entry) => ({ setting: entry.model, changes: entry.changes })),
        };
        for (const entry of batch) {
          entry.model.undo();
        }
        await this.refresh();
        return;
      case "invalid":
        if (this.#route(batch, failure)) {
          this.#schedule(0);
          return;
        }
        // No edit in the batch owns the problem, so say what allotd said before undoing them.
        this.writeProblem = failure.message;
        for (const entry of batch) {
          entry.model.undo();
        }
        await this.refresh();
        return;
      case "token":
      case "unreachable":
        this.#restore(batch);
        await this.#unanswered(failure);
        return;
      default:
        this.writeProblem = failure.message;
        // A held file is likely free again soon; any other failure waits for the user.
        this.#stalled = failure.kind !== "held";
        this.#restore(batch);
    }
  }

  /**
   * Shows each refused value's reason under its field, and sends the rest of the batch again.
   *
   * @param {Pending[]} batch
   * @param {SettingsFailure} failure
   * @returns {boolean} Whether any problem belonged to a change in the batch.
   */
  #route(batch, failure) {
    /** @type {Set<ValueModel<unknown>>} */
    const refused = new Set();
    for (const problem of failure.problems) {
      const entry = batch.find((candidate) => owns(candidate.model.key, problem.key));
      if (entry !== undefined) {
        entry.model.refuse(entry.model.describeProblem(problem.key, problem.message));
        refused.add(entry.model);
      }
    }
    if (refused.size === 0) {
      return false;
    }
    this.#restore(batch.filter((entry) => !refused.has(entry.model)));
    return true;
  }

  /**
   * Puts changes back in the burst, unless the user changed the same setting since.
   *
   * @param {Pending[]} batch
   */
  #restore(batch) {
    for (const entry of batch) {
      if (!this.#pending.has(entry.model.key)) {
        this.#pending.set(entry.model.key, entry);
      }
    }
  }

  /** @returns {Change[]} */
  #unsaved() {
    /** @type {Change[]} */
    const changes = [];
    for (const entry of this.#pending.values()) {
      changes.push(...entry.changes);
    }
    return changes;
  }

  /** @param {number} ms */
  #schedule(ms) {
    this.#clearSave();
    this.#saveTimer = this.#timers.setTimeout(() => {
      this.#saveTimer = null;
      void this.flush();
    }, ms);
  }

  #tick() {
    this.#refreshTimer = this.#timers.setTimeout(() => {
      this.#refreshTimer = null;
      if (this.#visible && !this.#disposed) {
        this.#tick();
      }
    }, REFRESH_INTERVAL_MS);
    this.refresh().catch((error) => {
      if (!(error instanceof CancelledError)) {
        throw error;
      }
    });
  }

  #clearSave() {
    if (this.#saveTimer !== null) {
      this.#timers.clearTimeout(this.#saveTimer);
      this.#saveTimer = null;
    }
  }

  #clearRefresh() {
    if (this.#refreshTimer !== null) {
      this.#timers.clearTimeout(this.#refreshTimer);
      this.#refreshTimer = null;
    }
  }

  #notify() {
    this.onChange?.();
  }
}

/**
 * Whether the setting `key` owns a problem reported for `problemKey`: the key itself, a row of
 * a table (`presets.big.min_cores`) or a rule (`classifier.rules[2].program`).
 *
 * @param {string} key
 * @param {string} problemKey
 */
function owns(key, problemKey) {
  return (
    problemKey === key || problemKey.startsWith(`${key}.`) || problemKey.startsWith(`${key}[`)
  );
}

/**
 * @param {PageModel[]} pages
 * @returns {ValueModel<unknown>[]}
 */
function changed(pages) {
  /** @type {ValueModel<unknown>[]} */
  const settings = [];
  for (const page of pages) {
    for (const group of page.groups) {
      settings.push(...group.settings.filter((model) => model.canReset));
    }
  }
  return settings;
}

/**
 * The stop record, as allotd reads it: a file that is not a record still means the user
 * stopped allotd, from the CLI, since only its presence counts.
 *
 * @param {string | null} text
 * @returns {StopRecord | null}
 */
export function parseStopRecord(text) {
  if (text === null) {
    return null;
  }
  try {
    return decodeStopRecord(JSON.parse(text));
  } catch {
    return { by: "cli", stopped_at_ms: 0 };
  }
}
