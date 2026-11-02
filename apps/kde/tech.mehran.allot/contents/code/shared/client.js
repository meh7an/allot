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
import { decodeKillResponse, decodeRemoveResponse, decodeUiState } from "../contract.g.mjs";
import * as stopResult from "./stop-result.js";
/** @import { UiState } from "../contract.g.mjs" */
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
/**
 * @typedef {object} HttpRequest
 * @property {"GET" | "POST"} method
 * @property {string} url
 * @property {string | null} token
 *   Sent as a bearer token when set.
 * @property {number} timeoutSecs
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
     * `POST /shutdown`, which allotd accepts before it stops.
     *
     * @returns {Promise<string | null>} `null` when allotd accepted, else why not. Rejects only
     *   with `CancelledError`.
     */
    shutdown() {
        return __awaiter(this, void 0, void 0, function* () {
            let response;
            try {
                response = yield this._sendWithToken("shutdown", "POST");
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
     * @param {"GET" | "POST"} [method]
     * @param {number} [timeoutSecs]
     * @returns {Promise<HttpResponse | null>} `null` when there is no token to send.
     */
    _sendWithToken(path_1) {
        return __awaiter(this, arguments, void 0, function* (path, method = "GET", timeoutSecs = REQUEST_TIMEOUT_SECS) {
            const token = yield this._currentToken();
            if (token === null) {
                return null;
            }
            const response = yield this._request(path, token, method, timeoutSecs);
            if (response.status !== 401) {
                return response;
            }
            // allotd writes a new token when its token file was deleted, and this client may still hold
            // the old one.
            this._token = null;
            const fresh = yield this._currentToken();
            return fresh === null ? null : this._request(path, fresh, method, timeoutSecs);
        });
    }
    /**
     * @param {string} path
     * @param {string | null} token
     * @param {"GET" | "POST"} [method]
     * @param {number} [timeoutSecs]
     * @returns {Promise<HttpResponse>}
     */
    _request(path, token, method = "GET", timeoutSecs = REQUEST_TIMEOUT_SECS) {
        return this._send({ method, url: this._base + path, token, timeoutSecs });
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
