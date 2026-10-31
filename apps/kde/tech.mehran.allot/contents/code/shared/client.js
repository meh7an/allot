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
var __classPrivateFieldSet = (this && this.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
    if (kind === "m") throw new TypeError("Private method is not writable");
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
    return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
};
var __classPrivateFieldGet = (this && this.__classPrivateFieldGet) || function (receiver, state, kind, f) {
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
    return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
};
var _DaemonClient_instances, _DaemonClient_base, _DaemonClient_send, _DaemonClient_readToken, _DaemonClient_token, _DaemonClient_withoutUiState, _DaemonClient_sendWithToken, _DaemonClient_request, _DaemonClient_currentToken;
import { decodeUiState } from "../contract.g.mjs";
/** @import { UiState } from "../contract.g.mjs" */
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
 * @property {"GET"} method
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
 * Sends one request. Rejects with `CancelledError` when the extension is being disabled, and
 * with any other error when nothing answers.
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
const REQUEST_TIMEOUT_SECS = 3;
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
        _DaemonClient_instances.add(this);
        _DaemonClient_base.set(this, void 0);
        _DaemonClient_send.set(this, void 0);
        _DaemonClient_readToken.set(this, void 0);
        /** @type {string | null} */
        _DaemonClient_token.set(this, null);
        __classPrivateFieldSet(this, _DaemonClient_base, `http://127.0.0.1:${options.port}/`, "f");
        __classPrivateFieldSet(this, _DaemonClient_send, options.send, "f");
        __classPrivateFieldSet(this, _DaemonClient_readToken, options.readToken, "f");
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
                response = yield __classPrivateFieldGet(this, _DaemonClient_instances, "m", _DaemonClient_sendWithToken).call(this, "ui/state");
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
                    return __classPrivateFieldGet(this, _DaemonClient_instances, "m", _DaemonClient_withoutUiState).call(this);
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
                response = yield __classPrivateFieldGet(this, _DaemonClient_instances, "m", _DaemonClient_request).call(this, "health", null);
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
}
_DaemonClient_base = new WeakMap(), _DaemonClient_send = new WeakMap(), _DaemonClient_readToken = new WeakMap(), _DaemonClient_token = new WeakMap(), _DaemonClient_instances = new WeakSet(), _DaemonClient_withoutUiState = function _DaemonClient_withoutUiState() {
    return __awaiter(this, void 0, void 0, function* () {
        const version = yield this.healthVersion();
        if (version === null) {
            return result("stopped", { detail: "GET /ui/state answered 404" });
        }
        return result("incompatible", { version, detail: `allotd ${version} has no /ui/state` });
    });
}, _DaemonClient_sendWithToken = function _DaemonClient_sendWithToken(path) {
    return __awaiter(this, void 0, void 0, function* () {
        const token = yield __classPrivateFieldGet(this, _DaemonClient_instances, "m", _DaemonClient_currentToken).call(this);
        if (token === null) {
            return null;
        }
        const response = yield __classPrivateFieldGet(this, _DaemonClient_instances, "m", _DaemonClient_request).call(this, path, token);
        if (response.status !== 401) {
            return response;
        }
        // allotd writes a new token when its token file was deleted, and this client may still hold
        // the old one.
        __classPrivateFieldSet(this, _DaemonClient_token, null, "f");
        const fresh = yield __classPrivateFieldGet(this, _DaemonClient_instances, "m", _DaemonClient_currentToken).call(this);
        return fresh === null ? null : __classPrivateFieldGet(this, _DaemonClient_instances, "m", _DaemonClient_request).call(this, path, fresh);
    });
}, _DaemonClient_request = function _DaemonClient_request(path, token) {
    return __classPrivateFieldGet(this, _DaemonClient_send, "f").call(this, {
        method: "GET",
        url: __classPrivateFieldGet(this, _DaemonClient_base, "f") + path,
        token,
        timeoutSecs: REQUEST_TIMEOUT_SECS,
    });
}, _DaemonClient_currentToken = function _DaemonClient_currentToken() {
    return __awaiter(this, void 0, void 0, function* () {
        var _a;
        var _b;
        if (__classPrivateFieldGet(this, _DaemonClient_token, "f") !== null) {
            return __classPrivateFieldGet(this, _DaemonClient_token, "f");
        }
        const read = (_b = (_a = (yield __classPrivateFieldGet(this, _DaemonClient_readToken, "f").call(this))) === null || _a === void 0 ? void 0 : _a.trim()) !== null && _b !== void 0 ? _b : "";
        __classPrivateFieldSet(this, _DaemonClient_token, read === "" ? null : read, "f");
        return __classPrivateFieldGet(this, _DaemonClient_token, "f");
    });
};
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
