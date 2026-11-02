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
import { CancelledError, TimeoutError } from "./shared/client.js";
/** @import { HttpRequest, HttpResponse } from "./shared/client.js" */
/** @import { Timers } from "./shared/poller.js" */
/** @import { ServiceSystem } from "./shared/service-launcher.js" */
// QML can neither read or write files, nor start programs, nor time out a request on its own, so
// the widget does those through Plasma's executable data engine and times requests with QML
// timers.
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
        /** @type {Map<string, ((output: CommandOutput) => void)[]>} */
        this._waiting = new Map();
        this._connect = connect;
    }
    /**
     * @param {string} command
     * @returns {Promise<CommandOutput>}
     */
    run(command) {
        return new Promise((resolve) => {
            const waiting = this._waiting.get(command);
            if (waiting !== undefined) {
                waiting.push(resolve);
                return;
            }
            this._waiting.set(command, [resolve]);
            this._connect(command);
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
        const waiting = (_a = this._waiting.get(command)) !== null && _a !== void 0 ? _a : [];
        this._waiting.delete(command);
        const output = {
            exitCode: Number(data["exit code"]),
            stdout: typeof data.stdout === "string" ? data.stdout : "",
        };
        for (const resolve of waiting) {
            resolve(output);
        }
    }
}
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
 * Replaces a small text file through a temporary file and a rename, so a crash never leaves half
 * a file.
 *
 * @param {CommandRunner} runner
 * @param {string} path
 * @param {string} text
 * @returns {Promise<boolean>} True when the file was written.
 */
export function writeTextFile(runner, path, text) {
    return __awaiter(this, void 0, void 0, function* () {
        const temporary = shellQuote(`${path}.tmp`);
        const folder = shellQuote(path.slice(0, path.lastIndexOf("/")));
        const output = yield runner.run(`mkdir -p -- ${folder} && printf %s ${shellQuote(text)} > ${temporary} && ` +
            `mv -f -- ${temporary} ${shellQuote(path)}`);
        return output.exitCode === 0;
    });
}
/**
 * Copies a file unless the target has the same contents already.
 *
 * @param {CommandRunner} runner
 * @param {string} source
 * @param {string} target
 * @returns {Promise<boolean>} True when the target is in place.
 */
export function copyFile(runner, source, target) {
    return __awaiter(this, void 0, void 0, function* () {
        const from = shellQuote(source);
        const to = shellQuote(target);
        const folder = shellQuote(target.slice(0, target.lastIndexOf("/")));
        const output = yield runner.run(`cmp -s -- ${from} ${to} || { mkdir -p -- ${folder} && cp -f -- ${from} ${to}; }`);
        return output.exitCode === 0;
    });
}
/**
 * A program that outlives the widget's call: its own session, so plasmashell's process group never
 * signals it, and from `/` with silent streams, as `allot-client` starts allotd.
 *
 * @param {string[]} argv
 * @returns {string}
 */
export function detachedCommand(argv) {
    return `cd / && setsid -f ${argv.map(shellQuote).join(" ")} < /dev/null > /dev/null 2>&1`;
}
/**
 * Start Service through the executable engine.
 *
 * @param {CommandRunner} runner
 * @returns {ServiceSystem}
 */
export function serviceSystem(runner) {
    return {
        readText: (path) => readTextFile(runner, path),
        isExecutable: (path) => __awaiter(this, void 0, void 0, function* () {
            const output = yield runner.run(`test -x ${shellQuote(path)}`);
            return output.exitCode === 0;
        }),
        run: (argv) => __awaiter(this, void 0, void 0, function* () {
            const command = argv.map(shellQuote).join(" ");
            const output = yield runner.run(`cd / && ${command} < /dev/null > /dev/null 2>&1`);
            return output.exitCode === 0;
        }),
        spawn: (path) => {
            runner.run(detachedCommand([path]));
        },
    };
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
        /** @type {Set<() => void>} */
        this._inFlight = new Set();
        /**
         * @param {HttpRequest} request
         * @returns {Promise<HttpResponse>}
         */
        this.send = (request) => new Promise((resolve, reject) => {
            const timers = this._timers;
            const inFlight = this._inFlight;
            const xhr = this._requests.create();
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
                abort(new TimeoutError());
            }, request.timeoutSecs * 1000);
            inFlight.add(cancel);
            xhr.open(request.method, request.url);
            if (request.token !== null) {
                xhr.setRequestHeader("Authorization", `Bearer ${request.token}`);
            }
            this._requests.send(xhr);
        });
        this._requests = requests;
        this._timers = timers;
    }
    cancel() {
        for (const cancel of [...this._inFlight]) {
            cancel();
        }
    }
}
