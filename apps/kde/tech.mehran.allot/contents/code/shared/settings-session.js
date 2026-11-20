// Generated from apps/gnome/allot@mehran.tech/lib/settings-session.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
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
    /**
     * @param {SettingsClient} client
     * @param {Timers} timers
     * @param {object} options
     * @param {Platform} options.platform Settings for other platforms are left out.
     * @param {() => Promise<string | null>} options.readStopRecord
     *   The text of `~/.allot/stopped.json`, or `null` when there is none.
     */
    constructor(client, timers, options) {
        /** @type {SessionStatus} */
        this.status = "loading";
        /** @type {string | null} */
        this.detail = null;
        /** @type {StopRecord | null} */
        this.stopRecord = null;
        /** @type {string | null} */
        this.fileError = null;
        /** @type {string[]} */
        this.pendingRestart = [];
        /** @type {number | null} */
        this.listeningPort = null;
        /** @type {PageModel[]} */
        this.pages = [];
        /** @type {Conflict | null} */
        this.conflict = null;
        /**
         * Why the last save was not written though allotd answered, such as a held file; the
         * changes stay unsaved until the next save.
         *
         * @type {string | null}
         */
        this.writeProblem = null;
        /** @type {(() => void) | null} */
        this.onChange = null;
        /** @type {string | null} */
        this._revision = null;
        /** @type {Map<string, ValueModel<unknown>>} */
        this._models = new Map();
        /** @type {Map<string, Pending>} */
        this._pending = new Map();
        /** @type {unknown} */
        this._saveTimer = null;
        /** @type {unknown} */
        this._refreshTimer = null;
        this._saving = false;
        this._visible = false;
        this._disposed = false;
        this._client = client;
        this._timers = timers;
        this._platform = options.platform;
        this._readStopRecord = options.readStopRecord;
    }
    /** Whether allotd's settings cannot be changed now. */
    get readOnly() {
        return this.status !== "ready" || this.fileError !== null;
    }
    /** @returns {SaveStatus} */
    get saveStatus() {
        if (this._saving) {
            return { kind: "saving" };
        }
        let count = this._pending.size;
        for (const model of this._models.values()) {
            if (model.issue !== null && !this._pending.has(model.key)) {
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
        return this._models.get(key);
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
    refresh() {
        return __awaiter(this, void 0, void 0, function* () {
            const reply = yield this._client.settings();
            if (this._disposed) {
                return;
            }
            if (reply.ok) {
                this._take(reply.value);
                if (this._pending.size > 0 && !this.readOnly) {
                    this._schedule(0);
                }
            }
            else {
                yield this._unanswered(reply.failure);
            }
            this._notify();
        });
    }
    /**
     * Starts or stops fetching every 5 s. Hiding the window sends a pending burst first.
     *
     * @param {boolean} visible
     */
    setVisible(visible) {
        this._visible = visible;
        this._clearRefresh();
        if (visible) {
            this._tick();
        }
        else if (this._pending.size > 0) {
            void this.flush();
        }
    }
    /**
     * Sends the pending changes now instead of after the burst.
     *
     * @returns {Promise<void>}
     */
    flush() {
        return __awaiter(this, void 0, void 0, function* () {
            this._clearSave();
            if (this._saving || this._pending.size === 0 || this.readOnly || this._revision === null) {
                this._notify();
                return;
            }
            const batch = [...this._pending.values()];
            this._pending.clear();
            this._saving = true;
            this._notify();
            /** @type {Change[]} */
            const changes = [];
            for (const entry of batch) {
                changes.push(...entry.changes);
            }
            let reply;
            try {
                reply = yield this._client.changeSettings({ revision: this._revision, changes });
            }
            catch (error) {
                if (error instanceof CancelledError) {
                    return;
                }
                throw error;
            }
            finally {
                this._saving = false;
            }
            if (this._disposed) {
                return;
            }
            if (reply.ok) {
                this.writeProblem = null;
                for (const entry of batch) {
                    entry.model.markSaved(entry.generation);
                }
                this._take(reply.value);
            }
            else {
                yield this._refused(batch, reply.failure);
            }
            if (this._pending.size > 0 && this._saveTimer === null) {
                this._schedule(SAVE_DELAY_MS);
            }
            this._notify();
        });
    }
    /** Sends the changes a hand edit got in the way of again, over the file's values. */
    applyMine() {
        const conflict = this.conflict;
        if (conflict === null) {
            return;
        }
        this.conflict = null;
        for (const { setting, changes } of conflict.edits) {
            this._pending.set(setting.key, { model: setting, changes, generation: setting.generation });
        }
        void this.flush();
    }
    /** Drops the changes a hand edit got in the way of; the file's values stay. */
    keepFileValues() {
        this.conflict = null;
        this._notify();
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
        return this._client.previewSettings([...this._unsaved(), ...changes]);
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
        return this._client.classify({ line, shell, changes: [...this._unsaved(), ...changes] });
    }
    /** Stops the timers; answers that arrive later are dropped. */
    dispose() {
        this._disposed = true;
        this._clearSave();
        this._clearRefresh();
    }
    /**
     * @param {string} key
     * @returns {unknown}
     */
    valueOf(key) {
        var _a;
        return (_a = this._models.get(key)) === null || _a === void 0 ? void 0 : _a.value;
    }
    /**
     * @param {string} key
     * @returns {string}
     */
    labelOf(key) {
        var _a;
        var _b;
        return (_b = (_a = this._models.get(key)) === null || _a === void 0 ? void 0 : _a.label) !== null && _b !== void 0 ? _b : key;
    }
    /**
     * @param {ValueModel<unknown>} model
     * @param {Change[]} changes
     * @param {boolean} now
     */
    edited(model, changes, now) {
        if (changes.length === 0) {
            this._pending.delete(model.key);
        }
        else {
            this._pending.set(model.key, { model, changes, generation: model.generation });
        }
        if (now) {
            void this.flush();
            return;
        }
        if (changes.length > 0) {
            this._schedule(SAVE_DELAY_MS);
        }
        this._notify();
    }
    /** @param {SettingsState} state */
    _take(state) {
        this.status = "ready";
        this.detail = null;
        this.stopRecord = null;
        this._revision = state.revision;
        this.fileError = state.file_error;
        this.pendingRestart = state.pending_restart;
        this.listeningPort = state.listening_port;
        /** @type {Map<string, unknown>} */
        const values = new Map();
        for (const entry of state.values) {
            values.set(entry.key, entry.value);
        }
        if (this._models.size === 0) {
            this.pages = this._build(state, values);
            return;
        }
        for (const model of this._models.values()) {
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
    _build(state, values) {
        /** @type {PageModel[]} */
        const pages = [];
        for (const page of state.spec.pages) {
            /** @type {GroupModel[]} */
            const groups = [];
            for (const group of page.groups) {
                const settings = group.settings
                    .filter((spec) => spec.platforms.length === 0 || spec.platforms.includes(this._platform))
                    .filter((spec) => values.has(spec.key))
                    .map((spec) => valueModel(spec, this, values.get(spec.key)));
                for (const model of settings) {
                    this._models.set(model.key, model);
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
    _unanswered(failure) {
        return __awaiter(this, void 0, void 0, function* () {
            this.detail = failure.message;
            switch (failure.kind) {
                case "unreachable":
                    this.stopRecord = parseStopRecord(yield this._readStopRecord());
                    this.status = this.stopRecord === null ? "down" : "stopped";
                    break;
                case "token":
                    this.status = "token";
                    break;
                default:
                    this.status = "failed";
            }
        });
    }
    /**
     * @param {Pending[]} batch
     * @param {SettingsFailure} failure
     */
    _refused(batch, failure) {
        return __awaiter(this, void 0, void 0, function* () {
            switch (failure.kind) {
                case "conflict":
                    this.conflict = {
                        edits: batch.map((entry) => ({ setting: entry.model, changes: entry.changes })),
                    };
                    for (const entry of batch) {
                        entry.model.undo();
                    }
                    yield this.refresh();
                    return;
                case "invalid":
                    if (this._route(batch, failure)) {
                        this._schedule(0);
                        return;
                    }
                    // The file itself is broken, so nothing can be saved until it is fixed by hand.
                    for (const entry of batch) {
                        entry.model.undo();
                    }
                    yield this.refresh();
                    return;
                case "token":
                case "unreachable":
                    this._restore(batch);
                    yield this._unanswered(failure);
                    return;
                default:
                    this.writeProblem = failure.message;
                    this._restore(batch);
            }
        });
    }
    /**
     * Shows each refused value's reason under its field, and sends the rest of the batch again.
     *
     * @param {Pending[]} batch
     * @param {SettingsFailure} failure
     * @returns {boolean} Whether any problem belonged to a change in the batch.
     */
    _route(batch, failure) {
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
        this._restore(batch.filter((entry) => !refused.has(entry.model)));
        return true;
    }
    /**
     * Puts changes back in the burst, unless the user changed the same setting since.
     *
     * @param {Pending[]} batch
     */
    _restore(batch) {
        for (const entry of batch) {
            if (!this._pending.has(entry.model.key)) {
                this._pending.set(entry.model.key, entry);
            }
        }
    }
    /** @returns {Change[]} */
    _unsaved() {
        /** @type {Change[]} */
        const changes = [];
        for (const entry of this._pending.values()) {
            changes.push(...entry.changes);
        }
        return changes;
    }
    /** @param {number} ms */
    _schedule(ms) {
        this._clearSave();
        this._saveTimer = this._timers.setTimeout(() => {
            this._saveTimer = null;
            void this.flush();
        }, ms);
    }
    _tick() {
        this._refreshTimer = this._timers.setTimeout(() => {
            this._refreshTimer = null;
            if (this._visible && !this._disposed) {
                this._tick();
            }
        }, REFRESH_INTERVAL_MS);
        this.refresh().catch((error) => {
            if (!(error instanceof CancelledError)) {
                throw error;
            }
        });
    }
    _clearSave() {
        if (this._saveTimer !== null) {
            this._timers.clearTimeout(this._saveTimer);
            this._saveTimer = null;
        }
    }
    _clearRefresh() {
        if (this._refreshTimer !== null) {
            this._timers.clearTimeout(this._refreshTimer);
            this._refreshTimer = null;
        }
    }
    _notify() {
        var _a;
        (_a = this.onChange) === null || _a === void 0 ? void 0 : _a.call(this);
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
    return (problemKey === key || problemKey.startsWith(`${key}.`) || problemKey.startsWith(`${key}[`));
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
    }
    catch (_a) {
        return { by: "cli", stopped_at_ms: 0 };
    }
}
