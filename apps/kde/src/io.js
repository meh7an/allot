import { CancelledError } from "../../gnome/allot@mehran.tech/lib/client.js";

/** @import { HttpRequest, HttpResponse } from "../../gnome/allot@mehran.tech/lib/client.js" */
/** @import { Timers } from "../../gnome/allot@mehran.tech/lib/poller.js" */

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
  #connect;
  /** @type {Map<string, ((output: CommandOutput) => void)[]>} */
  #waiting = new Map();

  /** @param {(command: string) => void} connect Starts a command; `deliver` gets its answer. */
  constructor(connect) {
    this.#connect = connect;
  }

  /**
   * @param {string} command
   * @returns {Promise<CommandOutput>}
   */
  run(command) {
    return new Promise((resolve) => {
      const waiting = this.#waiting.get(command);
      if (waiting !== undefined) {
        waiting.push(resolve);
        return;
      }
      this.#waiting.set(command, [resolve]);
      this.#connect(command);
    });
  }

  /**
   * Hands one answer of the data engine to the callers waiting for its command.
   *
   * @param {string} command The source name the engine answered for.
   * @param {Record<string, unknown>} data The engine's `exit code`, `stdout` and `stderr`.
   */
  deliver(command, data) {
    const waiting = this.#waiting.get(command) ?? [];
    this.#waiting.delete(command);
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
export async function readTextFile(runner, path) {
  const output = await runner.run(catCommand(path));
  return output.exitCode === 0 ? output.stdout : null;
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
  #requests;
  #timers;
  /** @type {Set<() => void>} */
  #inFlight = new Set();

  /**
   * @param {XhrFactory} requests
   * @param {Timers} timers
   */
  constructor(requests, timers) {
    this.#requests = requests;
    this.#timers = timers;
  }

  /**
   * @param {HttpRequest} request
   * @returns {Promise<HttpResponse>}
   */
  send = (request) =>
    new Promise((resolve, reject) => {
      const timers = this.#timers;
      const inFlight = this.#inFlight;
      const xhr = this.#requests.create();
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
        } else {
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
      this.#requests.send(xhr);
    });

  cancel() {
    for (const cancel of [...this.#inFlight]) {
      cancel();
    }
  }
}
