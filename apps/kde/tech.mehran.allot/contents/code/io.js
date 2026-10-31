// Generated from apps/kde/src/io.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
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
var _CommandRunner_connect, _CommandRunner_waiting, _XhrTransport_requests, _XhrTransport_timers, _XhrTransport_inFlight;
import { CancelledError } from "./shared/client.js";
/** @import { HttpRequest, HttpResponse } from "./shared/client.js" */
/** @import { Timers } from "./shared/poller.js" */
// QML can neither read files nor time out a request on its own, so the widget reads them
// through Plasma's executable data engine and times requests with QML timers.
/**
 * What the executable data engine hands back for one command.
 *
 * @typedef {object} CommandOutput
 * @property {number} exitCode
 * @property {string} stdout
 */
/**
 * The part of QML's XMLHttpRequest the transport uses.
 *
 * @typedef {object} Xhr
 * @property {number} readyState
 * @property {number} status
 * @property {string} responseText
 * @property {(() => void) | null} onreadystatechange
 * @property {(method: string, url: string) => void} open
 * @property {(name: string, value: string) => void} setRequestHeader
 * @property {() => void} send
 * @property {() => void} abort
 */
/**
 * Makes and sends requests. QML's XMLHttpRequest calls its handlers back only when `send` runs
 * in QML code, never in a module, so the widget's QML item does both.
 *
 * @typedef {object} XhrFactory
 * @property {() => Xhr} create
 * @property {(xhr: Xhr) => void} send
 */
const XHR_DONE = 4;
/**
 * Runs shell commands through the executable data engine. The engine answers by source name, so
 * callers that ask for the same command while it runs share its one answer.
 */
export class CommandRunner {
    /** @param {(command: string) => void} connect Starts a command; `deliver` gets its answer. */
    constructor(connect) {
        _CommandRunner_connect.set(this, void 0);
        /** @type {Map<string, ((output: CommandOutput) => void)[]>} */
        _CommandRunner_waiting.set(this, new Map());
        __classPrivateFieldSet(this, _CommandRunner_connect, connect, "f");
    }
    /**
     * @param {string} command
     * @returns {Promise<CommandOutput>}
     */
    run(command) {
        return new Promise((resolve) => {
            const waiting = __classPrivateFieldGet(this, _CommandRunner_waiting, "f").get(command);
            if (waiting !== undefined) {
                waiting.push(resolve);
                return;
            }
            __classPrivateFieldGet(this, _CommandRunner_waiting, "f").set(command, [resolve]);
            __classPrivateFieldGet(this, _CommandRunner_connect, "f").call(this, command);
        });
    }
    /**
     * Hands one answer of the data engine to the callers waiting for its command.
     *
     * @param {string} command The source name the engine answered for.
     * @param {Record<string, unknown>} data The engine's `exit code`, `stdout` and `stderr`.
     */
    deliver(command, data) {
        var _a;
        const waiting = (_a = __classPrivateFieldGet(this, _CommandRunner_waiting, "f").get(command)) !== null && _a !== void 0 ? _a : [];
        __classPrivateFieldGet(this, _CommandRunner_waiting, "f").delete(command);
        const output = {
            exitCode: Number(data["exit code"]),
            stdout: typeof data.stdout === "string" ? data.stdout : "",
        };
        for (const resolve of waiting) {
            resolve(output);
        }
    }
}
_CommandRunner_connect = new WeakMap(), _CommandRunner_waiting = new WeakMap();
/**
 * Reads a small text file.
 *
 * @param {CommandRunner} runner
 * @param {string} path
 * @returns {Promise<string | null>} `null` when the file cannot be read.
 */
export function readTextFile(runner, path) {
    return __awaiter(this, void 0, void 0, function* () {
        const output = yield runner.run(catCommand(path));
        return output.exitCode === 0 ? output.stdout : null;
    });
}
/**
 * @param {string} path
 * @returns {string}
 */
export function catCommand(path) {
    return `cat -- ${shellQuote(path)}`;
}
/**
 * Quotes one word for `sh`: inside single quotes only the quote itself needs escaping.
 *
 * @param {string} word
 * @returns {string}
 */
export function shellQuote(word) {
    return `'${word.replace(/'/g, "'\\''")}'`;
}
/**
 * The local path of a `file:` URL, as QML's `StandardPaths` returns them.
 *
 * @param {string} url
 * @returns {string}
 */
export function localPath(url) {
    return decodeURIComponent(String(url).replace(/^file:\/\//, ""));
}
/** Sends the client's requests with QML's XMLHttpRequest; `cancel` ends those in flight. */
export class XhrTransport {
    /**
     * @param {XhrFactory} requests
     * @param {Timers} timers
     */
    constructor(requests, timers) {
        _XhrTransport_requests.set(this, void 0);
        _XhrTransport_timers.set(this, void 0);
        /** @type {Set<() => void>} */
        _XhrTransport_inFlight.set(this, new Set());
        /**
         * @param {HttpRequest} request
         * @returns {Promise<HttpResponse>}
         */
        this.send = (request) => new Promise((resolve, reject) => {
            const timers = __classPrivateFieldGet(this, _XhrTransport_timers, "f");
            const inFlight = __classPrivateFieldGet(this, _XhrTransport_inFlight, "f");
            const xhr = __classPrivateFieldGet(this, _XhrTransport_requests, "f").create();
            /** @type {unknown} */
            let timer = null;
            function settle() {
                if (timer !== null) {
                    timers.clearTimeout(timer);
                    timer = null;
                }
                xhr.onreadystatechange = null;
                inFlight.delete(cancel);
            }
            /** @param {Error} error */
            function abort(error) {
                settle();
                xhr.abort();
                reject(error);
            }
            function cancel() {
                abort(new CancelledError());
            }
            xhr.onreadystatechange = () => {
                if (xhr.readyState !== XHR_DONE) {
                    return;
                }
                settle();
                if (xhr.status === 0) {
                    reject(new Error(`nothing answers at ${request.url}`));
                }
                else {
                    resolve({ status: xhr.status, body: xhr.responseText });
                }
            };
            timer = timers.setTimeout(() => {
                timer = null;
                abort(new Error(`no answer within ${request.timeoutSecs} s`));
            }, request.timeoutSecs * 1000);
            inFlight.add(cancel);
            xhr.open(request.method, request.url);
            if (request.token !== null) {
                xhr.setRequestHeader("Authorization", `Bearer ${request.token}`);
            }
            __classPrivateFieldGet(this, _XhrTransport_requests, "f").send(xhr);
        });
        __classPrivateFieldSet(this, _XhrTransport_requests, requests, "f");
        __classPrivateFieldSet(this, _XhrTransport_timers, timers, "f");
    }
    cancel() {
        for (const cancel of [...__classPrivateFieldGet(this, _XhrTransport_inFlight, "f")]) {
            cancel();
        }
    }
}
_XhrTransport_requests = new WeakMap(), _XhrTransport_timers = new WeakMap(), _XhrTransport_inFlight = new WeakMap();
