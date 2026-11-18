// The spike's one settings client, shared by GNOME's prefs.js and the KDE widget as the real
// panels share `lib/`. Written in ES2016 syntax by hand, without a build step: QML's engine has no
// class fields, private members or async functions.

/** The mock server's port; the real client reads `daemon.port` from `config.toml`. */
export const SPIKE_PORT = 47960;

/**
 * @typedef {object} Field
 * @property {string} key
 * @property {string} page Which page or group shows it.
 * @property {string} title
 * @property {string} subtitle
 * @property {"number" | "switch"} kind
 * @property {number} [min]
 * @property {number} [max]
 * @property {number} [step]
 * @property {boolean} [restart] Needs an allotd restart to take effect.
 */

/**
 * The settings the spike edits, one of each kind, as M1.1's spec will describe every key.
 *
 * @type {Field[]}
 */
export const SPIKE_FIELDS = [
  {
    key: "admission.headroom_pct",
    page: "admission",
    title: "Headroom",
    subtitle: "Percent of the machine kept free. 100 is refused, to try an invalid value.",
    kind: "number",
    min: 0,
    max: 100,
    step: 1,
  },
  {
    key: "estimates.lower",
    page: "admission",
    title: "Lower learned estimates",
    subtitle: "Shrink a job's estimate when its runs use less.",
    kind: "switch",
  },
  {
    key: "estimates.min_runs",
    page: "admission",
    title: "Runs before learning",
    subtitle: "Runs of a command before its estimate changes.",
    kind: "number",
    min: 1,
    max: 50,
    step: 1,
  },
  {
    key: "estimates.margin_pct",
    page: "admission",
    title: "Estimate margin",
    subtitle: "Percent added on top of a learned estimate.",
    kind: "number",
    min: 0,
    max: 100,
    step: 1,
  },
  {
    key: "leases.idle_release_secs",
    page: "leases",
    title: "Release idle leases after",
    subtitle: "Seconds without load before a lease is released.",
    kind: "number",
    min: 0,
    max: 86400,
    step: 30,
  },
  {
    key: "retention.history_days",
    page: "leases",
    title: "Keep history for",
    subtitle: "Days of run history kept.",
    kind: "number",
    min: 1,
    max: 3650,
    step: 1,
  },
  {
    key: "daemon.port",
    page: "service",
    title: "Port",
    subtitle: "Takes effect after allotd restarts.",
    kind: "number",
    min: 1024,
    max: 65535,
    step: 1,
    restart: true,
  },
];

/**
 * @typedef {object} HttpRequest
 * @property {"GET" | "PATCH"} method
 * @property {string} url
 * @property {string} token
 * @property {string | null} body JSON, sent with a JSON content type when set.
 */

/**
 * @typedef {object} HttpResponse
 * @property {number} status
 * @property {string} body
 */

/**
 * @typedef {object} Settings
 * @property {string} revision
 * @property {Record<string, number | boolean>} values
 * @property {Record<string, number | boolean>} defaults
 * @property {boolean} restart_pending
 */

/**
 * - `ok`: `settings` holds what allotd has now.
 * - `conflict`: the file changed since `revision`; `settings` holds the new state.
 * - `invalid`: `errors` names each refused key.
 * - `no-token`, `unauthorized`, `unreachable`, `failed`: `detail` says why.
 *
 * @typedef {object} Outcome
 * @property {"ok" | "conflict" | "invalid" | "no-token" | "unauthorized" | "unreachable" |
 *   "failed"} status
 * @property {Settings | null} settings
 * @property {Array<{ key: string, message: string }>} errors
 * @property {string} detail
 */

export class SettingsClient {
  /**
   * @param {object} options
   * @param {(request: HttpRequest) => Promise<HttpResponse>} options.send
   *   Rejects when nothing answers.
   * @param {() => Promise<string | null>} options.readToken `null` when there is no token file.
   * @param {number} [options.port]
   */
  constructor(options) {
    this._base = `http://127.0.0.1:${options.port === undefined ? SPIKE_PORT : options.port}/`;
    this._send = options.send;
    this._readToken = options.readToken;
    /** @type {string | null} */
    this._token = null;
  }

  /** @returns {Promise<Outcome>} */
  load() {
    return this._request("GET", null).then((response) =>
      response.status === 200 ? settingsOutcome("ok", response.body) : failure(response),
    );
  }

  /**
   * One write of every change, checked against the revision the page shows.
   *
   * @param {string} revision
   * @param {Record<string, number | boolean>} set
   * @param {string[]} reset
   * @returns {Promise<Outcome>}
   */
  apply(revision, set, reset) {
    const body = JSON.stringify({ revision, set, reset });
    return this._request("PATCH", body).then((response) => {
      switch (response.status) {
        case 200:
          return settingsOutcome("ok", response.body);
        case 409:
          return settingsOutcome("conflict", response.body);
        case 422:
          return outcome("invalid", { errors: JSON.parse(response.body).errors });
        default:
          return failure(response);
      }
    });
  }

  /**
   * Sends with the token, reading it again once after a 401, since allotd may have written a new
   * one. Resolves to a synthetic status 0 without a token and -1 when nothing answers.
   *
   * @param {"GET" | "PATCH"} method
   * @param {string | null} body
   * @returns {Promise<HttpResponse>}
   */
  _request(method, body) {
    const attempt = (fresh) =>
      this._tokenFor(fresh).then((token) => {
        if (token === null) {
          return { status: 0, body: "" };
        }
        return this._send({ method, url: `${this._base}settings`, token, body }).then(
          (response) => (response.status === 401 && !fresh ? attempt(true) : response),
          (error) => ({ status: -1, body: error instanceof Error ? error.message : `${error}` }),
        );
      });
    return attempt(this._token === null);
  }

  /**
   * @param {boolean} fresh
   * @returns {Promise<string | null>}
   */
  _tokenFor(fresh) {
    if (!fresh && this._token !== null) {
      return Promise.resolve(this._token);
    }
    return this._readToken().then((text) => {
      const token = text === null ? "" : text.trim();
      this._token = token === "" ? null : token;
      return this._token;
    });
  }
}

/**
 * @param {Outcome["status"]} status
 * @param {Partial<Outcome>} fields
 * @returns {Outcome}
 */
function outcome(status, fields) {
  return Object.assign({ status, settings: null, errors: [], detail: "" }, fields);
}

/**
 * @param {"ok" | "conflict"} status
 * @param {string} body
 * @returns {Outcome}
 */
function settingsOutcome(status, body) {
  try {
    return outcome(status, { settings: JSON.parse(body) });
  } catch (error) {
    return outcome("failed", { detail: `the answer is not JSON: ${error}` });
  }
}

/**
 * @param {HttpResponse} response
 * @returns {Outcome}
 */
function failure(response) {
  switch (response.status) {
    case 0:
      return outcome("no-token", { detail: "no token in ~/.allot/token" });
    case -1:
      return outcome("unreachable", { detail: response.body });
    case 401:
      return outcome("unauthorized", { detail: "allotd refused the token" });
    default:
      return outcome("failed", { detail: `allotd answered ${response.status}` });
  }
}

/**
 * The values in `edited` that differ from `settings`, for one PATCH.
 *
 * @param {Settings} settings
 * @param {Record<string, number | boolean>} edited
 * @returns {Record<string, number | boolean>}
 */
export function changedValues(settings, edited) {
  /** @type {Record<string, number | boolean>} */
  const changed = {};
  for (const key of Object.keys(edited)) {
    if (settings.values[key] !== edited[key]) {
      changed[key] = edited[key];
    }
  }
  return changed;
}
