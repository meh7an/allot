import Gio from "gi://Gio";
import GLib from "gi://GLib";

import { BusClient, BusRefusal } from "./lib/bus-client.js";
import { allotdAliveOffBus } from "./lib/off-bus.js";
import { CancelledError, TimeoutError } from "./lib/results.js";

/** @import { BusCall } from "./lib/bus-client.js" */
/** @import { Timers } from "./lib/poller.js" */

Gio._promisify(Gio.DBusConnection.prototype, "call");
Gio._promisify(Gio.File.prototype, "load_contents_async");
Gio._promisify(Gio.File.prototype, "replace_contents_bytes_async", "replace_contents_finish");
Gio._promisify(Gio.Subprocess.prototype, "communicate_utf8_async", "communicate_utf8_finish");

// allotd's D-Bus interface (`daemon/src/bus.rs`).
const NAME = "tech.mehran.Allot";
const PATH = "/tech/mehran/Allot";
const INTERFACE = "tech.mehran.Allot1";
const ERROR_PREFIX = `${INTERFACE}.Error.`;
const PROPERTIES = "org.freedesktop.DBus.Properties";
const NO_REPLY = "org.freedesktop.DBus.Error.NoReply";
const VERSION_TIMEOUT_MS = 3000;

/**
 * How the shell and the Settings window reach allotd: its interface on the session bus. It
 * watches allotd's name, so a start or a stop shows at once and nothing is called while allotd
 * is away, and it passes on `JobsChanged`.
 */
export class DaemonConnection {
  /**
   * Called when allotd takes or leaves its name on the bus.
   *
   * @type {(() => void) | null}
   */
  onOwnerChanged = null;

  /**
   * Called when allotd says a lease was queued, started, ended or killed.
   *
   * @type {(() => void) | null}
   */
  onJobsChanged = null;

  #cancellable = new Gio.Cancellable();
  /** @type {boolean | null} */
  #hasOwner = null;
  #watchId;
  #signalId;

  constructor() {
    this.#watchId = Gio.bus_watch_name(
      Gio.BusType.SESSION,
      NAME,
      Gio.BusNameWatcherFlags.NONE,
      () => this.#ownerChanged(true),
      () => this.#ownerChanged(false),
    );
    this.#signalId = Gio.DBus.session.signal_subscribe(
      NAME,
      INTERFACE,
      "JobsChanged",
      PATH,
      null,
      Gio.DBusSignalFlags.NONE,
      () => this.onJobsChanged?.(),
    );
  }

  /**
   * A client over this connection; every client of one connection shares its `cancel`.
   *
   * @param {() => Promise<boolean>} [offBus] Whether allotd runs though it is not on the bus.
   * @returns {BusClient}
   */
  client(offBus) {
    return new BusClient(
      {
        call: (call) => this.#call(call),
        version: () => this.#version(),
        hasOwner: () => this.#hasOwner,
      },
      offBus ?? null,
    );
  }

  /** Ends every call in flight, and stops watching allotd. */
  cancel() {
    this.#cancellable.cancel();
    this.stopWatching();
  }

  /** Stops watching allotd's name and signal; calls in flight still finish. */
  stopWatching() {
    if (this.#watchId !== 0) {
      Gio.bus_unwatch_name(this.#watchId);
      Gio.DBus.session.signal_unsubscribe(this.#signalId);
      this.#watchId = 0;
    }
  }

  /** @param {boolean} hasOwner */
  #ownerChanged(hasOwner) {
    this.#hasOwner = hasOwner;
    this.onOwnerChanged?.();
  }

  /**
   * @param {BusCall} call
   * @returns {Promise<string | null>}
   */
  async #call(call) {
    const args = call.signature === null ? null : new GLib.Variant(call.signature, call.args);
    const reply = await this.#send(INTERFACE, call.method, args, call.timeoutMs);
    const [json] = reply.recursiveUnpack();
    return typeof json === "string" ? json : null;
  }

  /** @returns {Promise<string>} */
  async #version() {
    const args = new GLib.Variant("(ss)", [INTERFACE, "Version"]);
    const reply = await this.#send(PROPERTIES, "Get", args, VERSION_TIMEOUT_MS);
    const [version] = reply.recursiveUnpack();
    return String(version);
  }

  /**
   * One method call to allotd. allotd is never started by the bus: Start service does that.
   *
   * @param {string} iface
   * @param {string} method
   * @param {GLib.Variant | null} args
   * @param {number} timeoutMs
   * @returns {Promise<GLib.Variant>}
   */
  async #send(iface, method, args, timeoutMs) {
    try {
      return await Gio.DBus.session.call(
        NAME,
        PATH,
        iface,
        method,
        args,
        null,
        Gio.DBusCallFlags.NO_AUTO_START,
        timeoutMs,
        this.#cancellable,
      );
    } catch (error) {
      throw this.#failure(error);
    }
  }

  /**
   * The error the client expects for a failed call.
   *
   * @param {unknown} error
   * @returns {unknown}
   */
  #failure(error) {
    if (this.#cancellable.is_cancelled()) {
      return new CancelledError();
    }
    if (!(error instanceof GLib.Error)) {
      return error;
    }
    if (error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.TIMED_OUT)) {
      return new TimeoutError();
    }
    const name = Gio.DBusError.get_remote_error(error);
    if (name === NO_REPLY) {
      return new TimeoutError();
    }
    if (name?.startsWith(ERROR_PREFIX)) {
      Gio.DBusError.strip_remote_error(error);
      return new BusRefusal(name.slice(ERROR_PREFIX.length), error.message);
    }
    return error;
  }
}

/**
 * Reads a small text file without blocking the compositor.
 *
 * @param {string} path
 * @param {Gio.Cancellable} cancellable
 * @returns {Promise<string | null>} `null` when the file cannot be read.
 */
export async function readTextFile(path, cancellable) {
  try {
    const [contents] = await Gio.File.new_for_path(path).load_contents_async(cancellable);
    return new TextDecoder().decode(contents);
  } catch {
    return null;
  }
}

/**
 * Replaces a small text file through a temporary file and a rename, so a crash never leaves half
 * a file. Not cancellable: locking the screen disables the extension, and a setting changed just
 * before must still be saved.
 *
 * @param {string} path
 * @param {string} text
 * @returns {Promise<void>}
 */
export async function writeTextFile(path, text) {
  await Gio.File.new_for_path(path).replace_contents_bytes_async(
    new GLib.Bytes(new TextEncoder().encode(text)),
    null,
    false,
    Gio.FileCreateFlags.REPLACE_DESTINATION,
    null,
  );
}

/**
 * Runs `systemctl` for Start Service, from `/`, keeping what it prints so a missing unit can be
 * told from a failure. Cancelling ends the wait only: `systemctl` exits by itself, and GLib
 * reaps it.
 *
 * @param {Gio.Cancellable} cancellable
 * @returns {import("./lib/service-launcher.js").ServiceSystem}
 */
export function createServiceSystem(cancellable) {
  return {
    readText: (path) => readTextFile(path, cancellable),
    run: async (argv) => {
      try {
        const launcher = new Gio.SubprocessLauncher({
          flags: Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_MERGE,
        });
        launcher.set_cwd("/");
        const process = launcher.spawnv(argv);
        const [output] = await process.communicate_utf8_async(null, cancellable);
        return { ok: process.get_successful(), output: output ?? "" };
      } catch (error) {
        return { ok: false, output: error instanceof Error ? error.message : String(error) };
      }
    },
  };
}

/**
 * Whether an allotd process is alive though it is not on the session bus, from the lock file it
 * writes its process id into.
 *
 * @param {string} root `~/.allot`.
 * @param {Gio.Cancellable} cancellable
 * @returns {() => Promise<boolean>}
 */
export function createOffBusProbe(root, cancellable) {
  return () => allotdAliveOffBus(root, (path) => readTextFile(path, cancellable));
}

/**
 * GLib timeouts for the poller. The poller clears only timers that have not fired, so
 * `source_remove` never meets a removed source.
 *
 * @type {Timers}
 */
export const glibTimers = {
  setTimeout: (callback, ms) =>
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
      callback();
      return GLib.SOURCE_REMOVE;
    }),
  clearTimeout: (id) => GLib.source_remove(/** @type {number} */ (id)),
};
