import { DESKTOP_ENTRIES_COMMAND } from "../src/app-icons.js";
import { INHIBITED_COMMAND } from "../src/notification-state.js";
import { AllotWidget } from "../src/widget.js";

/** @import { Xhr, XhrFactory } from "../src/io.js" */
/** @import { NotificationFields } from "../src/notifier.js" */
/** @import { Row, RowModel } from "../src/rows.js" */
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
 * QML's ListModel as the rows use it, with a count of the views it would have created.
 *
 * @implements {RowModel}
 */
export class FakeRowModel {
  /** @type {{ key: string, kind: string, leaving: boolean, entering: boolean }[]} */
  items = [];
  inserted = 0;

  /** @returns {number} */
  get count() {
    return this.items.length;
  }

  /** @returns {string[]} */
  get keys() {
    return this.items.map((item) => item.key);
  }

  /**
   * @param {string} key
   * @returns {boolean | undefined}
   */
  isLeaving(key) {
    return this.items.find((item) => item.key === key)?.leaving;
  }

  /**
   * @param {number} index
   * @returns {{ key: string, kind: string, leaving: boolean, entering: boolean }}
   */
  get(index) {
    const item = this.items[index];
    if (item === undefined) {
      throw new Error(`no row ${index}`);
    }
    return item;
  }

  /**
   * @param {number} index
   * @param {object} row
   */
  insert(index, row) {
    this.inserted += 1;
    this.items.splice(index, 0, /** @type {any} */ (row));
  }

  /**
   * @param {number} from
   * @param {number} to
   * @param {number} count
   */
  move(from, to, count) {
    this.items.splice(to, 0, ...this.items.splice(from, count));
  }

  /**
   * @param {number} index
   * @param {number} count
   */
  remove(index, count) {
    this.items.splice(index, count);
  }

  /**
   * @param {number} index
   * @param {string} role
   * @param {boolean} value
   */
  setProperty(index, role, value) {
    /** @type {any} */ (this.get(index))[role] = value;
  }
}

/**
 * A QML item stand-in: files for the executable engine to `cat`, a daemon for the requests, and
 * records of what the widget did.
 *
 * @param {object} options
 * @param {FakeTimers} options.timers
 * @param {Record<string, string>} [options.files] Contents by absolute path.
 * @param {(request: SentRequest) => Answer | undefined} options.daemon
 * @param {() => number} [options.now]
 * @param {string} [options.desktopEntries] What the desktop entries command prints.
 * @param {string} [options.inhibited] What the Do Not Disturb command prints; it fails without.
 * @param {Record<string, { "exit code": number, stdout: string }>} [options.answers]
 *   Answers to other commands by the command; a command with no answer exits 1.
 * @param {boolean} [options.copyFails] The notifyrc cannot be copied.
 * @param {boolean} [options.writeFails] The settings file cannot be written.
 */
export function fakeHost(options) {
  /** @type {string[]} */
  const commands = [];
  /** @type {FakeXhr[]} */
  const requests = [];
  /** @type {string[]} */
  const lines = [];
  const rows = new FakeRowModel();
  /** @type {{ data: Record<string, Row>, animate: boolean | null }} */
  const shown = { data: {}, animate: null };
  /** @type {FakeNotification[]} */
  const notifications = [];
  /** @type {(string | null)[]} */
  const scrolls = [];
  /** @type {string[]} */
  const opened = [];
  /** @type {string[]} */
  const announced = [];
  let expands = 0;
  let changes = 0;
  /** @type {AllotWidget | null} */
  let widget = null;

  /** @param {string} command */
  const answerTo = (command) => {
    if (command === DESKTOP_ENTRIES_COMMAND) {
      return { "exit code": 0, stdout: options.desktopEntries ?? "" };
    }
    if (command === INHIBITED_COMMAND) {
      return options.inhibited === undefined
        ? { "exit code": 1, stdout: "" }
        : { "exit code": 0, stdout: options.inhibited };
    }
    if (command.startsWith("cmp -s")) {
      return { "exit code": options.copyFails === true ? 1 : 0, stdout: "" };
    }
    if (command.startsWith("mkdir -p")) {
      return { "exit code": options.writeFails === true ? 1 : 0, stdout: "" };
    }
    return options.answers?.[command] ?? catAnswer(command, options.files ?? {});
  };

  /** @type {WidgetHost} */
  const host = {
    homeUrl: "file:///home/user%20one",
    configUrl: "file:///home/user%20one/.config",
    dataUrl: "file:///home/user%20one/.local/share",
    notifyRcUrl: "file:///package/contents/notifications/allot.notifyrc",
    connectSource: (command) => {
      commands.push(command);
      queueMicrotask(() => {
        widget?.deliver(command, answerTo(command));
      });
    },
    requests: fakeRequests(options.daemon, requests),
    timers: options.timers,
    rows,
    showRows: (data, animate) => {
      shown.data = data;
      shown.animate = animate;
    },
    onChange: () => {
      changes += 1;
    },
    sendNotification: (fields) => {
      const sent = new FakeNotification(fields);
      notifications.push(sent);
      return sent;
    },
    expand: () => {
      expands += 1;
      widget?.setExpanded(true);
    },
    scrollTo: (key) => scrolls.push(key),
    openUrl: (url) => opened.push(url),
    announce: (text) => announced.push(text),
    timeOfDay: (ms) => `t${ms}`,
    log: (line) => lines.push(line),
    ...(options.now === undefined ? {} : { now: options.now }),
  };
  widget = new AllotWidget(host);
  return {
    widget,
    timers: options.timers,
    commands,
    requests,
    lines,
    rows,
    shown,
    notifications,
    scrolls,
    opened,
    announced,
    get expands() {
      return expands;
    },
    get changes() {
      return changes;
    },
  };
}

/** A notification the widget sent, which a test can press or time out. */
export class FakeNotification {
  closedByWidget = false;
  fields;

  /** @param {NotificationFields} fields */
  constructor(fields) {
    this.fields = fields;
  }

  // Like Plasma's, closing one tells the sender asynchronously.
  close() {
    this.closedByWidget = true;
    queueMicrotask(() => this.fields.closed());
  }

  /** @param {string} label */
  press(label) {
    this.fields.buttons.find((button) => button.label === label)?.pressed();
  }
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
