import {
  decodeCommandTest,
  decodeFitPreview,
  decodeKillResponse,
  decodeRemoveResponse,
  decodeSettingsRefusal,
  decodeSettingsState,
} from "../../gnome/allot@mehran.tech/contract.g.mjs";
import {
  TimeoutError,
  conflictReason,
  pollResult,
  rethrowCancelled,
  runningState,
  settingsFailure,
  stoppedOrRethrow,
} from "../../gnome/allot@mehran.tech/lib/results.js";
import * as stopResult from "../../gnome/allot@mehran.tech/lib/stop-result.js";

/**
 * @import {
 *   Change,
 *   ClassifyRequest,
 *   CommandTest,
 *   FitPreview,
 *   SettingsPatch,
 *   SettingsState,
 *   ShutdownRequest,
 * } from "../../gnome/allot@mehran.tech/contract.g.mjs"
 */
/**
 * @import {
 *   PollResult,
 *   SettingsFailure,
 *   SettingsFailureKind,
 *   SettingsReply,
 * } from "../../gnome/allot@mehran.tech/lib/results.js"
 */
/** @import { StopResult } from "../../gnome/allot@mehran.tech/lib/stop-result.js" */

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
 * Sends one request. Rejects with `CancelledError` when the widget is being closed, with
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

const REQUEST_TIMEOUT_SECS = 3;

// allotd answers a kill after up to three sweeps over the tree, each waiting out the grace
// period of the processes that ignore SIGTERM.
const STOP_TIMEOUT_SECS = 10;

/** @type {ShutdownRequest} */
const PANEL_STOP = { by: "panel" };

/**
 * The widget's HTTP client for allotd's routes, with the token from `~/.allot/token`. It never
 * starts allotd by itself; that is the user's Start service.
 */
export class DaemonClient {
  #port;
  #base;
  #send;
  #readToken;
  /** @type {string | null} */
  #token = null;

  /**
   * @param {object} options
   * @param {number} options.port allotd's loopback port.
   * @param {SendRequest} options.send
   * @param {ReadText} options.readToken Read on first use and after a 401.
   */
  constructor(options) {
    this.#port = options.port;
    this.#base = `http://127.0.0.1:${options.port}/`;
    this.#send = options.send;
    this.#readToken = options.readToken;
  }

  /**
   * `GET /ui/state`, mapped to a service status.
   *
   * @returns {Promise<PollResult>} Rejects only with `CancelledError`.
   */
  async poll() {
    let response;
    try {
      response = await this.#sendWithToken("ui/state");
    } catch (error) {
      return stoppedOrRethrow(error);
    }
    if (response === null) {
      return pollResult("stopped", { detail: "no token in ~/.allot/token" });
    }
    switch (response.status) {
      case 200:
        return runningState(response.body);
      case 404:
        return this.#withoutUiState();
      case 503:
        return pollResult("starting", { detail: "the daemon has no first sample yet" });
      case 401:
        return pollResult("incompatible", { detail: "the daemon refused the token" });
      default:
        return pollResult("incompatible", {
          detail: `GET /ui/state answered ${response.status}`,
        });
    }
  }

  /**
   * `GET /health`, which needs no token; the version of a healthy daemon, or `null` when nothing
   * healthy answers.
   *
   * @returns {Promise<string | null>} Rejects only with `CancelledError`.
   */
  async healthVersion() {
    let response;
    try {
      response = await this.#request("health", null);
    } catch (error) {
      rethrowCancelled(error);
      return null;
    }
    if (response.status !== 200) {
      return null;
    }
    try {
      const health = JSON.parse(response.body);
      return typeof health?.version === "string" ? health.version : null;
    } catch {
      return null;
    }
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
  async shutdown(options = {}) {
    const body = options.record === false ? undefined : PANEL_STOP;
    let response;
    try {
      response = await this.#sendWithToken("shutdown", "POST", REQUEST_TIMEOUT_SECS, body);
    } catch (error) {
      rethrowCancelled(error);
      return error instanceof Error ? error.message : String(error);
    }
    if (response === null) {
      return "no token in ~/.allot/token";
    }
    return response.status === 202 ? null : `POST /shutdown answered ${response.status}`;
  }

  /**
   * Sends every later request to `port`, such as after a restart on a new port.
   *
   * @param {number} port
   */
  usePort(port) {
    this.#port = port;
    this.#base = `http://127.0.0.1:${port}/`;
  }

  /** The port requests go to. */
  get port() {
    return this.#port;
  }

  /**
   * `GET /settings`: every setting with its spec, value and the file's revision.
   *
   * @returns {Promise<SettingsReply<SettingsState>>} Rejects only with `CancelledError`.
   */
  settings() {
    return this.#settingsRoute("settings", "GET", undefined, decodeSettingsState);
  }

  /**
   * `PATCH /settings`: writes the changes together or not at all.
   *
   * @param {SettingsPatch} patch
   * @returns {Promise<SettingsReply<SettingsState>>} The new state. Rejects only with
   *   `CancelledError`.
   */
  changeSettings(patch) {
    return this.#settingsRoute("settings", "PATCH", patch, decodeSettingsState);
  }

  /**
   * `POST /settings/preview`: what fits with unsaved changes on top of the file.
   *
   * @param {Change[]} changes
   * @returns {Promise<SettingsReply<FitPreview>>} Rejects only with `CancelledError`.
   */
  previewSettings(changes) {
    return this.#settingsRoute("settings/preview", "POST", { changes }, decodeFitPreview);
  }

  /**
   * `POST /classify`: how the hook treats a command line under the file's rules, or under
   * unsaved changes to them.
   *
   * @param {ClassifyRequest} request
   * @returns {Promise<SettingsReply<CommandTest>>} Rejects only with `CancelledError`.
   */
  classify(request) {
    return this.#settingsRoute("classify", "POST", request, decodeCommandTest);
  }

  /**
   * `POST /leases/{id}/kill`.
   *
   * @param {number} lease
   * @returns {Promise<StopResult>} Rejects only with `CancelledError`.
   */
  kill(lease) {
    return this.#stop(`leases/${lease}/kill`, (body) =>
      stopResult.killed(decodeKillResponse(body)),
    );
  }

  /**
   * `POST /leases/{id}/remove`.
   *
   * @param {number} lease
   * @returns {Promise<StopResult>} Rejects only with `CancelledError`.
   */
  remove(lease) {
    return this.#stop(`leases/${lease}/remove`, (body) =>
      stopResult.removed(decodeRemoveResponse(body)),
    );
  }

  /**
   * @param {string} path
   * @param {(body: unknown) => StopResult} ended Reads allotd's answer; throws when it cannot.
   * @returns {Promise<StopResult>}
   */
  async #stop(path, ended) {
    let response;
    try {
      response = await this.#sendWithToken(path, "POST", STOP_TIMEOUT_SECS);
    } catch (error) {
      rethrowCancelled(error);
      return stopResult.refused(
        error instanceof TimeoutError
          ? `the service did not answer within ${STOP_TIMEOUT_SECS} s`
          : "the service is not running",
      );
    }
    if (response === null) {
      return stopResult.refused("the service's token file is missing");
    }
    if (response.status !== 200) {
      return stopResult.refused(stopRefusal(response.status, response.body.trim()));
    }
    try {
      return ended(JSON.parse(response.body));
    } catch {
      return stopResult.refused("the service's answer could not be read");
    }
  }

  /**
   * @template T
   * @param {string} path
   * @param {HttpMethod} method
   * @param {object | undefined} body
   * @param {(json: unknown) => T} decodeValue Throws when the answer is not a `T`.
   * @returns {Promise<SettingsReply<T>>}
   */
  async #settingsRoute(path, method, body, decodeValue) {
    let response;
    try {
      response = await this.#sendWithToken(path, method, REQUEST_TIMEOUT_SECS, body);
    } catch (error) {
      rethrowCancelled(error);
      return settingsFailure("unreachable", error instanceof Error ? error.message : String(error));
    }
    if (response === null) {
      return settingsFailure("key", "no token in ~/.allot/token");
    }
    if (response.status === 200) {
      try {
        return { ok: true, value: decodeValue(JSON.parse(response.body)) };
      } catch (error) {
        return settingsFailure("failed", `the service's answer could not be read: ${error}`);
      }
    }
    if (response.status === 401) {
      return settingsFailure("key", "the service refused the token");
    }
    return { ok: false, failure: settingsRefusal(response) };
  }

  // allotd v0.1 has no /ui/state, and answers 404 for it.
  async #withoutUiState() {
    const version = await this.healthVersion();
    if (version === null) {
      return pollResult("stopped", { detail: "GET /ui/state answered 404" });
    }
    return pollResult("incompatible", { version, detail: `allotd ${version} has no /ui/state` });
  }

  /**
   * @param {string} path
   * @param {HttpMethod} [method]
   * @param {number} [timeoutSecs]
   * @param {object} [body] Sent as JSON.
   * @returns {Promise<HttpResponse | null>} `null` when there is no token to send.
   */
  async #sendWithToken(path, method = "GET", timeoutSecs = REQUEST_TIMEOUT_SECS, body) {
    const token = await this.#currentToken();
    if (token === null) {
      return null;
    }
    const json = body === undefined ? undefined : JSON.stringify(body);
    const response = await this.#request(path, token, method, timeoutSecs, json);
    if (response.status !== 401) {
      return response;
    }

    // allotd writes a new token when its token file was deleted, and this client may still hold
    // the old one.
    this.#token = null;
    const fresh = await this.#currentToken();
    return fresh === null ? null : this.#request(path, fresh, method, timeoutSecs, json);
  }

  /**
   * @param {string} path
   * @param {string | null} token
   * @param {HttpMethod} [method]
   * @param {number} [timeoutSecs]
   * @param {string} [body]
   * @returns {Promise<HttpResponse>}
   */
  #request(path, token, method = "GET", timeoutSecs = REQUEST_TIMEOUT_SECS, body) {
    /** @type {HttpRequest} */
    const request = { method, url: this.#base + path, token, timeoutSecs };
    if (body !== undefined) {
      request.body = body;
    }
    return this.#send(request);
  }

  /** @returns {Promise<string | null>} */
  async #currentToken() {
    if (this.#token !== null) {
      return this.#token;
    }
    const read = (await this.#readToken())?.trim() ?? "";
    this.#token = read === "" ? null : read;
    return this.#token;
  }
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
  const kind = REFUSAL_KINDS[response.status] ?? "failed";
  try {
    const refusal = decodeSettingsRefusal(JSON.parse(response.body));
    return {
      kind,
      message: refusal.message,
      problems: refusal.problems,
      revision: refusal.revision,
    };
  } catch {
    const text = response.body.trim();
    return {
      kind,
      message: text === "" ? `the service answered ${response.status}` : text,
      problems: [],
      revision: null,
    };
  }
}
