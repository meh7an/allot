// Generated from apps/gnome/allot@mehran.tech/lib/results.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
import { decodeUiState } from "../contract.g.mjs";
/** @import { FieldError, UiState } from "../contract.g.mjs" */
/**
 * - `running`: allotd answered with UI state.
 * - `starting`: allotd answers but has no first sample yet.
 * - `stopped`: nothing answers, or the panel has no way to ask yet.
 * - `incompatible`: allotd answers but cannot serve this panel: an older build, a state shape
 *   the generated types reject, or a caller it refuses. Starting it again does not help;
 *   reinstalling does.
 *
 * @typedef {"running" | "starting" | "stopped" | "incompatible"} ServiceStatus
 */
/**
 * The outcome of one poll.
 *
 * @typedef {object} PollResult
 * @property {ServiceStatus} status
 * @property {UiState | null} state
 *   The UI state; only set while `running`.
 * @property {string | null} version
 *   The daemon version, when it is known.
 * @property {string | null} detail
 *   Why the service is not running, for logs and the service-down view.
 */
/**
 * Why a settings call did not answer as asked:
 *
 * - `conflict`: `config.toml` changed since its revision was read; `revision` is the file's now.
 * - `invalid`: nothing written. With `problems`, the values that were refused (or the file's
 *   own problems); without, the file is not TOML or not UTF-8.
 * - `held`: another program holds `config.toml`.
 * - `unavailable`: allotd has no machine sample or no config file yet.
 * - `key`: the caller has no key, or allotd refused it after one fresh read.
 * - `unreachable`: nothing answered in time.
 * - `failed`: anything else, such as an answer that does not decode.
 *
 * @typedef {"conflict" | "invalid" | "held" | "unavailable" | "key" | "unreachable" | "failed"}
 *   SettingsFailureKind
 */
/**
 * @typedef {object} SettingsFailure
 * @property {SettingsFailureKind} kind
 * @property {string} message
 *   allotd's sentence, or what went wrong on the way.
 * @property {FieldError[]} problems
 * @property {string | null} revision
 */
/**
 * The answer of a settings call: the decoded value, or why there is none.
 *
 * @template T
 * @typedef {{ ok: true, value: T } | { ok: false, failure: SettingsFailure }} SettingsReply
 */
/** A request was cancelled; it must never read as a stopped service. */
export class CancelledError extends Error {
    constructor() {
        super("cancelled");
        this.name = "CancelledError";
    }
}
/** No answer came within the request's timeout. */
export class TimeoutError extends Error {
    constructor() {
        super("timed out");
        this.name = "TimeoutError";
    }
}
const NOT_KILLED = " was not killed: ";
/**
 * The reason in an error message of a kill or remove, for a card that already names the job:
 * allotd's text starts with the lease.
 *
 * @param {string} body
 * @returns {string}
 */
export function conflictReason(body) {
    const cut = body.indexOf(NOT_KILLED);
    if (cut >= 0) {
        return body.slice(cut + NOT_KILLED.length);
    }
    if (body.includes(" has already ended")) {
        return "the job has already ended";
    }
    return body === "" ? "the service refused" : body;
}
/**
 * A settings call that got no answer from allotd's settings code.
 *
 * @param {SettingsFailureKind} kind
 * @param {string} message
 * @returns {{ ok: false, failure: SettingsFailure }}
 */
export function settingsFailure(kind, message) {
    return { ok: false, failure: { kind, message, problems: [], revision: null } };
}
/**
 * The poll of a `UiState` answer: running, or incompatible when it does not decode.
 *
 * @param {string} body
 * @returns {PollResult}
 */
export function runningState(body) {
    try {
        const state = decodeUiState(JSON.parse(body));
        return pollResult("running", { state, version: state.service.version });
    }
    catch (error) {
        return pollResult("incompatible", { detail: String(error) });
    }
}
/**
 * Nothing answered, which reads as stopped; a cancel is passed on.
 *
 * @param {unknown} error
 * @returns {PollResult}
 */
export function stoppedOrRethrow(error) {
    rethrowCancelled(error);
    const detail = error instanceof Error ? error.message : String(error);
    return pollResult("stopped", { detail });
}
/**
 * A cancel must never read as a stopped service.
 *
 * @param {unknown} error
 */
export function rethrowCancelled(error) {
    if (error instanceof CancelledError) {
        throw error;
    }
}
/**
 * @param {ServiceStatus} status
 * @param {{ state?: UiState, version?: string, detail?: string }} [fields]
 * @returns {PollResult}
 */
export function pollResult(status, fields = {}) {
    var _a, _b, _c;
    return {
        status,
        state: (_a = fields.state) !== null && _a !== void 0 ? _a : null,
        version: (_b = fields.version) !== null && _b !== void 0 ? _b : null,
        detail: (_c = fields.detail) !== null && _c !== void 0 ? _c : null,
    };
}
