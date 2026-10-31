import { AllotWidget } from "../src/widget.js";

/** @import { Xhr, XhrFactory } from "../src/io.js" */
/** @import { WidgetHost } from "../src/widget.js" */
/** @import { FakeTimers } from "../../gnome/tests/support.mjs" */

export const KDE_DIR = new URL("../", import.meta.url);
export const PACKAGE_DIR = new URL("tech.mehran.allot/", KDE_DIR);

/**
 * An answer to a request: a status and body, or `null` for a connection nothing accepts.
 *
 * @typedef {{ status: number, body: string } | null} Answer
 */

/** @typedef {{ method: string, url: string, headers: Record<string, string> }} SentRequest */

/** An XMLHttpRequest that answers from a handler once a test lets promises run. */
export class FakeXhr {
  readyState = 0;
  status = 0;
  responseText = "";
  /** @type {(() => void) | null} */
  onreadystatechange = null;
  aborted = false;
  /** @type {SentRequest} */
  sent = { method: "", url: "", headers: {} };

  #answer;

  /** @param {(request: SentRequest) => Answer | undefined} answer `undefined` never answers. */
  constructor(answer) {
    this.#answer = answer;
  }

  /**
   * @param {string} method
   * @param {string} url
   */
  open(method, url) {
    this.sent.method = method;
    this.sent.url = url;
  }

  /**
   * @param {string} name
   * @param {string} value
   */
  setRequestHeader(name, value) {
    this.sent.headers[name] = value;
  }

  send() {
    const answer = this.#answer(this.sent);
    if (answer === undefined) {
      return;
    }
    queueMicrotask(() => {
      if (this.aborted) {
        return;
      }
      this.status = answer === null ? 0 : answer.status;
      this.responseText = answer === null ? "" : answer.body;
      this.readyState = 4;
      this.onreadystatechange?.();
    });
  }

  // Like QML's, an abort ends the request as done with status 0.
  abort() {
    this.aborted = true;
    this.readyState = 4;
    this.status = 0;
    this.onreadystatechange?.();
  }
}

/**
 * Requests answered by `answer`, each recorded in `made`.
 *
 * @param {(request: SentRequest) => Answer | undefined} answer
 * @param {FakeXhr[]} [made]
 * @returns {XhrFactory}
 */
export function fakeRequests(answer, made = []) {
  return {
    create: () => {
      const xhr = new FakeXhr(answer);
      made.push(xhr);
      return /** @type {Xhr} */ (/** @type {unknown} */ (xhr));
    },
    send: (xhr) => xhr.send(),
  };
}

/**
 * A QML item stand-in: files for the executable engine to `cat`, a daemon for the requests, and
 * records of what the widget did.
 *
 * @param {object} options
 * @param {FakeTimers} options.timers
 * @param {Record<string, string>} [options.files] Contents by absolute path.
 * @param {(request: SentRequest) => Answer | undefined} options.daemon
 */
export function fakeHost(options) {
  /** @type {string[]} */
  const commands = [];
  /** @type {FakeXhr[]} */
  const requests = [];
  /** @type {string[]} */
  const lines = [];
  let changes = 0;
  /** @type {AllotWidget | null} */
  let widget = null;

  /** @type {WidgetHost} */
  const host = {
    homeUrl: "file:///home/user%20one",
    connectSource: (command) => {
      commands.push(command);
      queueMicrotask(() => widget?.deliver(command, catAnswer(command, options.files ?? {})));
    },
    requests: fakeRequests(options.daemon, requests),
    timers: options.timers,
    onChange: () => {
      changes += 1;
    },
    log: (line) => lines.push(line),
  };
  widget = new AllotWidget(host);
  return {
    widget,
    timers: options.timers,
    commands,
    requests,
    lines,
    get changes() {
      return changes;
    },
  };
}

/**
 * What the executable engine answers for `cat -- '<path>'`.
 *
 * @param {string} command
 * @param {Record<string, string>} files
 * @returns {Record<string, unknown>}
 */
export function catAnswer(command, files) {
  const match = /^cat -- '(.*)'$/.exec(command);
  const path = match?.[1]?.replace(/'\\''/g, "'");
  const contents = path === undefined ? undefined : files[path];
  if (contents === undefined) {
    return { "exit code": 1, "exit status": 0, stdout: "", stderr: "cat: No such file" };
  }
  return { "exit code": 0, "exit status": 0, stdout: contents, stderr: "" };
}
