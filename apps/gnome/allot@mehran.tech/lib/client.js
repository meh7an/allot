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
      return result("stopped", { detail: "no token in ~/.allot/token" });
    }
    switch (response.status) {
      case 200:
        return decode(response.body);
      case 404:
        return this.#withoutUiState();
      case 503:
        return result("starting", { detail: "the daemon has no first sample yet" });
      case 401:
        return result("incompatible", { detail: "the daemon refused the token" });
      default:
        return result("incompatible", {
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

  // allotd v0.1 has no /ui/state, and answers 404 for it.
  async #withoutUiState() {
    const version = await this.healthVersion();
    if (version === null) {
      return result("stopped", { detail: "GET /ui/state answered 404" });
    }
    return result("incompatible", { version, detail: `allotd ${version} has no /ui/state` });
  }

  /**
   * @param {string} path
   * @returns {Promise<HttpResponse | null>} `null` when there is no token to send.
   */
  async #sendWithToken(path) {
    const token = await this.#currentToken();
    if (token === null) {
      return null;
    }
    const response = await this.#request(path, token);
    if (response.status !== 401) {
      return response;
    }

    // allotd writes a new token when its token file was deleted, and this client may still hold
    // the old one.
    this.#token = null;
    const fresh = await this.#currentToken();
    return fresh === null ? null : this.#request(path, fresh);
  }

  /**
   * @param {string} path
   * @param {string | null} token
   * @returns {Promise<HttpResponse>}
   */
  #request(path, token) {
    return this.#send({
      method: "GET",
      url: this.#base + path,
      token,
      timeoutSecs: REQUEST_TIMEOUT_SECS,
    });
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
 * @param {string} body
 * @returns {PollResult}
 */
function decode(body) {
  try {
    const state = decodeUiState(JSON.parse(body));
    return result("running", { state, version: state.service.version });
  } catch (error) {
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
  return {
    status,
    state: fields.state ?? null,
    version: fields.version ?? null,
    detail: fields.detail ?? null,
  };
}
