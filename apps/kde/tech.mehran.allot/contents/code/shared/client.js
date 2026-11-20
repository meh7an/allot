// Generated from apps/gnome/allot@mehran.tech/lib/client.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { decodeCommandTest, decodeFitPreview, decodeKillResponse, decodeRemoveResponse, decodeSettingsRefusal, decodeSettingsState, decodeUiState, } from "../contract.g.mjs";
import * as stopResult from "./stop-result.js";
/**
 * @import {
 *   Change,
 *   ClassifyRequest,
 *   CommandTest,
 *   FieldError,
 *   FitPreview,
 *   SettingsPatch,
 *   SettingsState,
 *   ShutdownRequest,
 *   UiState,
 * } from "../contract.g.mjs"
 */
/** @import { StopResult } from "./stop-result.js" */
/**
 * - `running`: allotd answered with UI state.
 * - `starting`: allotd answers but has no first sample yet.
 * - `stopped`: nothing answers on the port, or no token exists yet.
 * - `incompatible`: allotd answers but cannot serve this extension: an older build without
 *   `/ui/state`, a state shape the generated types reject, or a token it refuses. Starting it
 *   again does not help; reinstalling does.
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
/** @typedef {"GET" | "POST" | "PATCH"} HttpMethod */
/**
 * @typedef {object} HttpRequest
 * @property {HttpMethod} method
 * @property {string} url
 * @property {string | null} token
 *   Sent as a bearer token when set.
 * @property {number} timeoutSecs
 * @property {string} [body]
 *   JSON, sent with its content type, when the route takes a body.
 */
/**
 * @typedef {object} HttpResponse
 * @property {number} status
 * @property {string} body
 */
/**
 * Sends one request. Rejects with `CancelledError` when the extension is being disabled, with
 * `TimeoutError` when nothing answers in time, and with any other error when nothing answers.
 *
 * @callback SendRequest
 * @param {HttpRequest} request
 * @returns {Promise<HttpResponse>}
 */
/**
 * Reads the token file; `null` when it does not exist.
 *
 * @callback ReadText
 * @returns {Promise<string | null>}
 */
/**
 * Why a settings route did not answer as asked:
 *
 * - `conflict`: 409, `config.toml` changed since its revision was read; `revision` is the
 *   file's now.
 * - `invalid`: 422, nothing written. With `problems`, the values that were refused (or the
 *   file's own problems); without, the file is not TOML or not UTF-8.
 * - `held`: 423, another program holds `config.toml`.
 * - `unavailable`: 503, allotd has no machine sample or no config file yet.
 * - `token`: no token file, or allotd refused the token after one fresh read.
 * - `unreachable`: nothing answered in time.
 * - `failed`: anything else, such as a 500 or an answer that does not decode.
 *
 * @typedef {"conflict" | "invalid" | "held" | "unavailable" | "token" | "unreachable" | "failed"}
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
 * The answer of a settings route: the decoded value, or why there is none.
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
const REQUEST_TIMEOUT_SECS = 3;
// allotd answers a kill after up to three sweeps over the tree, each waiting out the grace
// period of the processes that ignore SIGTERM.
const STOP_TIMEOUT_SECS = 10;
const NOT_KILLED = " was not killed: ";
/** @type {ShutdownRequest} */
const PANEL_STOP = { by: "panel" };
/**
 * HTTP client for the daemon routes the extension uses. It never starts allotd by itself; that
 * is the user's Start service.
 */
export class DaemonClient {
    /**
     * @param {object} options
     * @param {number} options.port allotd's loopback port.
     * @param {SendRequest} options.send
     * @param {ReadText} options.readToken Read on first use and after a 401.
     */
    constructor(options) {
        /** @type {string | null} */
        this._token = null;
        this._port = options.port;
        this._base = `http://127.0.0.1:${options.port}/`;
        this._send = options.send;
        this._readToken = options.readToken;
    }
    /**
     * `GET /ui/state`, mapped to a service status.
     *
     * @returns {Promise<PollResult>} Rejects only with `CancelledError`.
     */
    poll() {
        return __awaiter(this, void 0, void 0, function* () {
            let response;
            try {
                response = yield this._sendWithToken("ui/state");
            }
            catch (error) {
                return stoppedOrRethrow(error);
            }
            if (response === null) {
                return result("stopped", { detail: "no token in ~/.allot/token" });
            }
            switch (response.status) {
                case 200:
                    return decode(response.body);
                case 404:
                    return this._withoutUiState();
                case 503:
                    return result("starting", { detail: "the daemon has no first sample yet" });
                case 401:
                    return result("incompatible", { detail: "the daemon refused the token" });
                default:
                    return result("incompatible", {
                        detail: `GET /ui/state answered ${response.status}`,
                    });
            }
        });
    }
    /**
     * `GET /health`, which needs no token; the version of a healthy daemon, or `null` when nothing
     * healthy answers.
     *
     * @returns {Promise<string | null>} Rejects only with `CancelledError`.
     */
    healthVersion() {
        return __awaiter(this, void 0, void 0, function* () {
            let response;
            try {
                response = yield this._request("health", null);
            }
            catch (error) {
                rethrowCancelled(error);
                return null;
            }
            if (response.status !== 200) {
                return null;
            }
            try {
                const health = JSON.parse(response.body);
                return typeof (health === null || health === void 0 ? void 0 : health.version) === "string" ? health.version : null;
            }
            catch (_a) {
                return null;
            }
        });
    }
    /**
     * `POST /shutdown` from Stop service: allotd writes the stop record, so no session starts it
     * again, then accepts and stops. Start service needs nothing of the kind, since every start of
     * allotd clears the record. A restart sends no body, so no record is written.
     *
     * @param {object} [options]
     * @param {boolean} [options.record] Whether allotd writes the stop record; true unless a
     *   restart follows.
     * @returns {Promise<string | null>} `null` when allotd accepted, else why not. Rejects only
     *   with `CancelledError`.
     */
    shutdown() {
        return __awaiter(this, arguments, void 0, function* (options = {}) {
            const body = options.record === false ? undefined : PANEL_STOP;
            let response;
            try {
                response = yield this._sendWithToken("shutdown", "POST", REQUEST_TIMEOUT_SECS, body);
            }
            catch (error) {
                rethrowCancelled(error);
                return error instanceof Error ? error.message : String(error);
            }
            if (response === null) {
                return "no token in ~/.allot/token";
            }
            return response.status === 202 ? null : `POST /shutdown answered ${response.status}`;
        });
    }
    /**
     * Sends every later request to `port`, such as after a restart on a new port.
     *
     * @param {number} port
     */
    usePort(port) {
        this._port = port;
        this._base = `http://127.0.0.1:${port}/`;
    }
    /** The port requests go to. */
    get port() {
        return this._port;
    }
    /**
     * `GET /settings`: every setting with its spec, value and the file's revision.
     *
     * @returns {Promise<SettingsReply<SettingsState>>} Rejects only with `CancelledError`.
     */
    settings() {
        return this._settingsRoute("settings", "GET", undefined, decodeSettingsState);
    }
    /**
     * `PATCH /settings`: writes the changes together or not at all.
     *
     * @param {SettingsPatch} patch
     * @returns {Promise<SettingsReply<SettingsState>>} The new state. Rejects only with
     *   `CancelledError`.
     */
    changeSettings(patch) {
        return this._settingsRoute("settings", "PATCH", patch, decodeSettingsState);
    }
    /**
     * `POST /settings/preview`: what fits with unsaved changes on top of the file.
     *
     * @param {Change[]} changes
     * @returns {Promise<SettingsReply<FitPreview>>} Rejects only with `CancelledError`.
     */
    previewSettings(changes) {
        return this._settingsRoute("settings/preview", "POST", { changes }, decodeFitPreview);
    }
    /**
     * `POST /classify`: how the hook treats a command line under the file's rules, or under
     * unsaved changes to them.
     *
     * @param {ClassifyRequest} request
     * @returns {Promise<SettingsReply<CommandTest>>} Rejects only with `CancelledError`.
     */
    classify(request) {
        return this._settingsRoute("classify", "POST", request, decodeCommandTest);
    }
    /**
     * `POST /leases/{id}/kill`.
     *
     * @param {number} lease
     * @returns {Promise<StopResult>} Rejects only with `CancelledError`.
     */
    kill(lease) {
        return this._stop(`leases/${lease}/kill`, (body) => stopResult.killed(decodeKillResponse(body)));
    }
    /**
     * `POST /leases/{id}/remove`.
     *
     * @param {number} lease
     * @returns {Promise<StopResult>} Rejects only with `CancelledError`.
     */
    remove(lease) {
        return this._stop(`leases/${lease}/remove`, (body) => stopResult.removed(decodeRemoveResponse(body)));
    }
    /**
     * @param {string} path
     * @param {(body: unknown) => StopResult} ended Reads allotd's answer; throws when it cannot.
     * @returns {Promise<StopResult>}
     */
    _stop(path, ended) {
        return __awaiter(this, void 0, void 0, function* () {
            let response;
            try {
                response = yield this._sendWithToken(path, "POST", STOP_TIMEOUT_SECS);
            }
            catch (error) {
                rethrowCancelled(error);
                return stopResult.refused(error instanceof TimeoutError
                    ? `the service did not answer within ${STOP_TIMEOUT_SECS} s`
                    : "the service is not running");
            }
            if (response === null) {
                return stopResult.refused("the service's token file is missing");
            }
            if (response.status !== 200) {
                return stopResult.refused(stopRefusal(response.status, response.body.trim()));
            }
            try {
                return ended(JSON.parse(response.body));
            }
            catch (_a) {
                return stopResult.refused("the service's answer could not be read");
            }
        });
    }
    /**
     * @template T
     * @param {string} path
     * @param {HttpMethod} method
     * @param {object | undefined} body
     * @param {(json: unknown) => T} decodeValue Throws when the answer is not a `T`.
     * @returns {Promise<SettingsReply<T>>}
     */
    _settingsRoute(path, method, body, decodeValue) {
        return __awaiter(this, void 0, void 0, function* () {
            let response;
            try {
                response = yield this._sendWithToken(path, method, REQUEST_TIMEOUT_SECS, body);
            }
            catch (error) {
                rethrowCancelled(error);
                return settingsFailure("unreachable", error instanceof Error ? error.message : String(error));
            }
            if (response === null) {
                return settingsFailure("token", "no token in ~/.allot/token");
            }
            if (response.status === 200) {
                try {
                    return { ok: true, value: decodeValue(JSON.parse(response.body)) };
                }
                catch (error) {
                    return settingsFailure("failed", `the service's answer could not be read: ${error}`);
                }
            }
            if (response.status === 401) {
                return settingsFailure("token", "the service refused the token");
            }
            return { ok: false, failure: settingsRefusal(response) };
        });
    }
    // allotd v0.1 has no /ui/state, and answers 404 for it.
    _withoutUiState() {
        return __awaiter(this, void 0, void 0, function* () {
            const version = yield this.healthVersion();
            if (version === null) {
                return result("stopped", { detail: "GET /ui/state answered 404" });
            }
            return result("incompatible", { version, detail: `allotd ${version} has no /ui/state` });
        });
    }
    /**
     * @param {string} path
     * @param {HttpMethod} [method]
     * @param {number} [timeoutSecs]
     * @param {object} [body] Sent as JSON.
     * @returns {Promise<HttpResponse | null>} `null` when there is no token to send.
     */
    _sendWithToken(path_1) {
        return __awaiter(this, arguments, void 0, function* (path, method = "GET", timeoutSecs = REQUEST_TIMEOUT_SECS, body) {
            const token = yield this._currentToken();
            if (token === null) {
                return null;
            }
            const json = body === undefined ? undefined : JSON.stringify(body);
            const response = yield this._request(path, token, method, timeoutSecs, json);
            if (response.status !== 401) {
                return response;
            }
            // allotd writes a new token when its token file was deleted, and this client may still hold
            // the old one.
            this._token = null;
            const fresh = yield this._currentToken();
            return fresh === null ? null : this._request(path, fresh, method, timeoutSecs, json);
        });
    }
    /**
     * @param {string} path
     * @param {string | null} token
     * @param {HttpMethod} [method]
     * @param {number} [timeoutSecs]
     * @param {string} [body]
     * @returns {Promise<HttpResponse>}
     */
    _request(path, token, method = "GET", timeoutSecs = REQUEST_TIMEOUT_SECS, body) {
        /** @type {HttpRequest} */
        const request = { method, url: this._base + path, token, timeoutSecs };
        if (body !== undefined) {
            request.body = body;
        }
        return this._send(request);
    }
    /** @returns {Promise<string | null>} */
    _currentToken() {
        return __awaiter(this, void 0, void 0, function* () {
            var _a;
            var _b;
            if (this._token !== null) {
                return this._token;
            }
            const read = (_b = (_a = (yield this._readToken())) === null || _a === void 0 ? void 0 : _a.trim()) !== null && _b !== void 0 ? _b : "";
            this._token = read === "" ? null : read;
            return this._token;
        });
    }
}
/**
 * The reason in an error body of the kill or remove route, for a card that already names the
 * job: allotd's text starts with the lease.
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
 * @param {number} status
 * @param {string} body
 * @returns {string}
 */
function stopRefusal(status, body) {
    switch (status) {
        case 409:
            return conflictReason(body);
        case 404:
            return "the job no longer exists";
        case 503:
            return "the service has just started; try again";
        case 401:
            return "the service refused the token";
        default:
            return `the service answered ${status}`;
    }
}
/**
 * @param {SettingsFailureKind} kind
 * @param {string} message
 * @returns {{ ok: false, failure: SettingsFailure }}
 */
function settingsFailure(kind, message) {
    return { ok: false, failure: { kind, message, problems: [], revision: null } };
}
/** @type {Readonly<Record<number, SettingsFailureKind>>} */
const REFUSAL_KINDS = { 409: "conflict", 422: "invalid", 423: "held", 503: "unavailable" };
/**
 * A refused settings request. Settings routes refuse with a `SettingsRefusal`; a body that is
 * not one, such as a 500's plain text, keeps its text as the message.
 *
 * @param {HttpResponse} response
 * @returns {SettingsFailure}
 */
function settingsRefusal(response) {
    var _a;
    const kind = (_a = REFUSAL_KINDS[response.status]) !== null && _a !== void 0 ? _a : "failed";
    try {
        const refusal = decodeSettingsRefusal(JSON.parse(response.body));
        return {
            kind,
            message: refusal.message,
            problems: refusal.problems,
            revision: refusal.revision,
        };
    }
    catch (_b) {
        const text = response.body.trim();
        return {
            kind,
            message: text === "" ? `the service answered ${response.status}` : text,
            problems: [],
            revision: null,
        };
    }
}
/**
 * @param {string} body
 * @returns {PollResult}
 */
function decode(body) {
    try {
        const state = decodeUiState(JSON.parse(body));
        return result("running", { state, version: state.service.version });
    }
    catch (error) {
        return result("incompatible", { detail: String(error) });
    }
}
/**
 * @param {unknown} error
 * @returns {PollResult}
 */
function stoppedOrRethrow(error) {
    rethrowCancelled(error);
    return result("stopped", { detail: error instanceof Error ? error.message : String(error) });
}
/** @param {unknown} error */
function rethrowCancelled(error) {
    if (error instanceof CancelledError) {
        throw error;
    }
}
/**
 * @param {ServiceStatus} status
 * @param {{ state?: UiState, version?: string, detail?: string }} [fields]
 * @returns {PollResult}
 */
function result(status, fields = {}) {
    var _a, _b, _c;
    return {
        status,
        state: (_a = fields.state) !== null && _a !== void 0 ? _a : null,
        version: (_b = fields.version) !== null && _b !== void 0 ? _b : null,
        detail: (_c = fields.detail) !== null && _c !== void 0 ? _c : null,
    };
}
