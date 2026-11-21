import {
  decodeCommandTest,
  decodeFitPreview,
  decodeKillResponse,
  decodeRemoveResponse,
  decodeSettingsRefusal,
  decodeSettingsState,
} from "../contract.g.mjs";
import { OFF_BUS_DETAIL } from "./off-bus.js";
import {
  TimeoutError,
  conflictReason,
  pollResult,
  rethrowCancelled,
  runningState,
  settingsFailure,
} from "./results.js";
import * as stopResult from "./stop-result.js";

/**
 * @import {
 *   Change,
 *   ClassifyRequest,
 *   CommandTest,
 *   FitPreview,
 *   SettingsPatch,
 *   SettingsState,
 * } from "../contract.g.mjs"
 */
/** @import { PollResult, SettingsFailureKind, SettingsReply } from "./results.js" */
/** @import { StopResult } from "./stop-result.js" */

/**
 * One call of allotd's D-Bus interface, `tech.mehran.Allot1`.
 *
 * @typedef {object} BusCall
 * @property {string} method
 * @property {string | null} signature
 *   The arguments' D-Bus signature, such as `(x)`; `null` for none.
 * @property {unknown[]} args
 * @property {number} timeoutMs
 */

/**
 * The session bus as the client needs it: `Gio.DBus` in the shell and the Settings window, a
 * fake in tests.
 *
 * - `call` resolves with the method's JSON answer, or `null` for a method without one. It
 *   rejects with `BusRefusal` when allotd refuses, `CancelledError` when the extension is being
 *   disabled, `TimeoutError` when nothing answers in time, and any other error when allotd is
 *   not on the bus.
 * - `version` reads allotd's `Version` property and rejects like `call`.
 * - `hasOwner` is whether allotd owns its name, as the name watcher saw last; `null` before
 *   the watcher's first answer.
 *
 * @typedef {object} Bus
 * @property {(call: BusCall) => Promise<string | null>} call
 * @property {() => Promise<string>} version
 * @property {() => boolean | null} hasOwner
 */

/** An error allotd answered, such as `Conflict`, with its message. */
export class BusRefusal extends Error {
  /**
   * @param {string} name The error's name after `tech.mehran.Allot1.Error.`.
   * @param {string} message
   */
  constructor(name, message) {
    super(message);
    this.name = name;
  }
}

const REQUEST_TIMEOUT_MS = 3000;

// A kill ends a whole process tree, which takes longer than a read.
const STOP_TIMEOUT_MS = 10_000;

/** @type {Readonly<Record<string, SettingsFailureKind>>} */
const REFUSAL_KINDS = {
  Conflict: "conflict",
  Invalid: "invalid",
  Held: "held",
  Unavailable: "unavailable",
};

const NOT_ON_BUS = "the service is not on the session bus";

// GDBus error text names bus internals, so it never reaches the user; this stands in for it.
const RETRY_LATER = "the service could not answer; see logs/allotd.log";

/**
 * The GNOME extension's client of allotd over D-Bus, for the shell and the Settings window. The
 * session bus says who calls, so it sends no key. It never starts allotd by itself; that is the
 * user's Start service.
 */
export class BusClient {
  #bus;
  #offBus;

  /**
   * @param {Bus} bus
   * @param {(() => Promise<boolean>) | null} [offBus] Whether an allotd process is alive though
   *   it is not on the bus; without one, an allotd off the bus reads as stopped.
   */
  constructor(bus, offBus = null) {
    this.#bus = bus;
    this.#offBus = offBus;
  }

  /**
   * `GetState`, mapped to a service status. While allotd is not on the bus it answers without
   * a call.
   *
   * @returns {Promise<PollResult>} Rejects only with `CancelledError`.
   */
  async poll() {
    if (this.#bus.hasOwner() === false) {
      return this.#goneFromBus(NOT_ON_BUS);
    }
    let json;
    try {
      json = await this.#call("GetState");
    } catch (error) {
      if (error instanceof BusRefusal) {
        switch (error.name) {
          case "Unavailable":
            return pollResult("starting", { detail: "the daemon has no first sample yet" });
          case "Failed":
            // allotd answered with an error of its own, which a later poll may not repeat;
            // reinstalling would not help.
            return pollResult("starting", { detail: RETRY_LATER });
          default:
            return pollResult("incompatible", {
              detail: `GetState: ${error.name}: ${error.message}`,
            });
        }
      }
      rethrowCancelled(error);
      const detail = unreachableText(error, REQUEST_TIMEOUT_MS);
      // A timeout means allotd owns its name and is busy, which is not an allotd off the bus.
      return error instanceof TimeoutError
        ? pollResult("stopped", { detail })
        : this.#goneFromBus(detail);
    }
    return runningState(json ?? "");
  }

  /**
   * The poll of a bus with no allotd on it: off the bus when its process is alive, else stopped.
   *
   * @param {string} detail
   * @returns {Promise<PollResult>}
   */
  async #goneFromBus(detail) {
    return (await this.#offBus?.()) === true
      ? pollResult("off_bus", { detail: OFF_BUS_DETAIL })
      : pollResult("stopped", { detail });
  }

  /**
   * allotd's version, or `null` while it is not on the bus.
   *
   * @returns {Promise<string | null>} Rejects only with `CancelledError`.
   */
  async healthVersion() {
    if (this.#bus.hasOwner() === false) {
      return null;
    }
    try {
      return await this.#bus.version();
    } catch (error) {
      rethrowCancelled(error);
      return null;
    }
  }

  /**
   * `Shutdown`. Stop service writes the stop record, so no session starts allotd again; a
   * restart passes `record: false`.
   *
   * @param {object} [options]
   * @param {boolean} [options.record] Whether allotd writes the stop record; true unless a
   *   restart follows.
   * @returns {Promise<string | null>} `null` when allotd accepted, else why not. Rejects only
   *   with `CancelledError`.
   */
  async shutdown(options = {}) {
    try {
      await this.#call("Shutdown", "(b)", [options.record !== false]);
      return null;
    } catch (error) {
      rethrowCancelled(error);
      return stopRefusal(error, REQUEST_TIMEOUT_MS, NOT_ON_BUS);
    }
  }

  /** @returns {Promise<SettingsReply<SettingsState>>} Rejects only with `CancelledError`. */
  settings() {
    return this.#settingsCall("GetSettings", null, decodeSettingsState);
  }

  /**
   * @param {SettingsPatch} patch
   * @returns {Promise<SettingsReply<SettingsState>>} Rejects only with `CancelledError`.
   */
  changeSettings(patch) {
    return this.#settingsCall("ChangeSettings", patch, decodeSettingsState);
  }

  /**
   * @param {Change[]} changes
   * @returns {Promise<SettingsReply<FitPreview>>} Rejects only with `CancelledError`.
   */
  previewSettings(changes) {
    return this.#settingsCall("PreviewSettings", { changes }, decodeFitPreview);
  }

  /**
   * @param {ClassifyRequest} request
   * @returns {Promise<SettingsReply<CommandTest>>} Rejects only with `CancelledError`.
   */
  classify(request) {
    return this.#settingsCall("Classify", request, decodeCommandTest);
  }

  /**
   * @param {number} lease
   * @returns {Promise<StopResult>} Rejects only with `CancelledError`.
   */
  kill(lease) {
    return this.#stop("Kill", lease, (json) => stopResult.killed(decodeKillResponse(json)));
  }

  /**
   * @param {number} lease
   * @returns {Promise<StopResult>} Rejects only with `CancelledError`.
   */
  remove(lease) {
    return this.#stop("Remove", lease, (json) => stopResult.removed(decodeRemoveResponse(json)));
  }

  /**
   * @param {string} method
   * @param {number} lease
   * @param {(json: unknown) => StopResult} ended Reads allotd's answer; throws when it cannot.
   * @returns {Promise<StopResult>}
   */
  async #stop(method, lease, ended) {
    let json;
    try {
      json = await this.#call(method, "(x)", [lease], STOP_TIMEOUT_MS);
    } catch (error) {
      rethrowCancelled(error);
      return stopResult.refused(stopRefusal(error));
    }
    try {
      return ended(JSON.parse(json ?? ""));
    } catch {
      return stopResult.refused("the service's answer could not be read");
    }
  }

  /**
   * @template T
   * @param {string} method
   * @param {object | null} request Sent as JSON.
   * @param {(json: unknown) => T} decodeValue Throws when the answer is not a `T`.
   * @returns {Promise<SettingsReply<T>>}
   */
  async #settingsCall(method, request, decodeValue) {
    let json;
    try {
      json =
        request === null
          ? await this.#call(method)
          : await this.#call(method, "(s)", [JSON.stringify(request)]);
    } catch (error) {
      rethrowCancelled(error);
      if (error instanceof BusRefusal) {
        return { ok: false, failure: settingsRefusal(error) };
      }
      if (!(error instanceof TimeoutError) && (await this.#offBus?.()) === true) {
        return settingsFailure("off_bus", OFF_BUS_DETAIL);
      }
      return settingsFailure("unreachable", unreachableText(error, REQUEST_TIMEOUT_MS));
    }
    try {
      return { ok: true, value: decodeValue(JSON.parse(json ?? "")) };
    } catch (error) {
      return settingsFailure("failed", `the service's answer could not be read: ${error}`);
    }
  }

  /**
   * @param {string} method
   * @param {string | null} [signature]
   * @param {unknown[]} [args]
   * @param {number} [timeoutMs]
   * @returns {Promise<string | null>}
   */
  #call(method, signature = null, args = [], timeoutMs = REQUEST_TIMEOUT_MS) {
    return this.#bus.call({ method, signature, args, timeoutMs });
  }
}

/**
 * Why a call got no answer, without the bus's own error text.
 *
 * @param {unknown} error
 * @param {number} timeoutMs What the call waited.
 * @returns {string}
 */
function unreachableText(error, timeoutMs) {
  return error instanceof TimeoutError
    ? `the service did not answer within ${timeoutMs / 1000} s`
    : NOT_ON_BUS;
}

/**
 * Why a kill, remove or shutdown did not take effect, for a card or row that already names it.
 *
 * @param {unknown} error
 * @param {number} [timeoutMs] What the call waited.
 * @param {string} [gone] The text for an error that is not allotd's answer.
 * @returns {string}
 */
function stopRefusal(error, timeoutMs = STOP_TIMEOUT_MS, gone = "the service is not running") {
  if (error instanceof TimeoutError) {
    return unreachableText(error, timeoutMs);
  }
  if (!(error instanceof BusRefusal)) {
    return gone;
  }
  switch (error.name) {
    case "Conflict":
      return conflictReason(error.message);
    case "NotFound":
      return "the job no longer exists";
    case "Unavailable":
      return "the service has just started; try again";
    default:
      return error.message === "" ? "the service refused" : error.message;
  }
}

/**
 * A refused settings call. Its message is a `SettingsRefusal` as JSON; one that is not keeps
 * its text as the message.
 *
 * @param {BusRefusal} refusal
 * @returns {import("./results.js").SettingsFailure}
 */
function settingsRefusal(refusal) {
  const kind = REFUSAL_KINDS[refusal.name] ?? "failed";
  try {
    const decoded = decodeSettingsRefusal(JSON.parse(refusal.message));
    return {
      kind,
      message: decoded.message,
      problems: decoded.problems,
      revision: decoded.revision,
    };
  } catch {
    return { kind, message: refusal.message, problems: [], revision: null };
  }
}
