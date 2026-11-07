import Gio from "gi://Gio";
import GLib from "gi://GLib";
import Soup from "gi://Soup?version=3.0";

import { CancelledError, TimeoutError } from "./lib/client.js";

/** @import { HttpRequest, HttpResponse } from "./lib/client.js" */
/** @import { Timers } from "./lib/poller.js" */

Gio._promisify(Soup.Session.prototype, "send_and_read_async");
Gio._promisify(Gio.File.prototype, "load_contents_async");
Gio._promisify(Gio.File.prototype, "replace_contents_bytes_async", "replace_contents_finish");
Gio._promisify(Gio.File.prototype, "query_info_async");
Gio._promisify(Gio.Subprocess.prototype, "wait_check_async");

/** Sends the client's requests with libsoup; `cancel` ends every one in flight. */
export class SoupTransport {
  // One session per timeout: a session's timeout applies to all its requests, and a poll must
  // not shorten the wait of a kill in flight.
  /** @type {Map<number, Soup.Session>} */
  #sessions = new Map();
  #cancellable = new Gio.Cancellable();

  /**
   * @param {HttpRequest} request
   * @returns {Promise<HttpResponse>}
   */
  send = async (request) => {
    const message = Soup.Message.new(request.method, request.url);
    if (request.token !== null) {
      message.request_headers.append("Authorization", `Bearer ${request.token}`);
    }
    try {
      const bytes = await this.#session(request.timeoutSecs).send_and_read_async(
        message,
        GLib.PRIORITY_DEFAULT,
        this.#cancellable,
      );
      const body = new TextDecoder().decode(bytes.get_data() ?? new Uint8Array());
      return { status: message.get_status(), body };
    } catch (error) {
      if (this.#cancellable.is_cancelled()) {
        throw new CancelledError();
      }
      if (
        error instanceof GLib.Error &&
        error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.TIMED_OUT)
      ) {
        throw new TimeoutError();
      }
      throw error;
    }
  };

  cancel() {
    this.#cancellable.cancel();
    for (const session of this.#sessions.values()) {
      session.abort();
    }
  }

  /**
   * @param {number} timeoutSecs
   * @returns {Soup.Session}
   */
  #session(timeoutSecs) {
    let session = this.#sessions.get(timeoutSecs);
    if (session === undefined) {
      // allotd only listens on loopback, which a system proxy must never see.
      session = new Soup.Session({
        proxy_resolver: Gio.SimpleProxyResolver.new(null, null),
        timeout: timeoutSecs,
      });
      this.#sessions.set(timeoutSecs, session);
    }
    return session;
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
 * Starts programs for Start Service, from `/` with no standard streams, as `allot-client` does.
 * Cancelling ends the waits only: `systemctl` exits by itself, and GLib reaps it.
 *
 * @param {Gio.Cancellable} cancellable
 * @returns {import("./lib/service-launcher.js").ServiceSystem}
 */
export function createServiceSystem(cancellable) {
  return {
    readText: (path) => readTextFile(path, cancellable),
    isExecutable: async (path) => {
      try {
        const info = await Gio.File.new_for_path(path).query_info_async(
          "access::can-execute",
          Gio.FileQueryInfoFlags.NONE,
          GLib.PRIORITY_DEFAULT,
          cancellable,
        );
        return info.get_attribute_boolean("access::can-execute");
      } catch {
        return false;
      }
    },
    run: async (argv) => {
      try {
        return await silentLauncher().spawnv(argv).wait_check_async(cancellable);
      } catch {
        return false;
      }
    },
    spawn: (path) => {
      // Its own session, so the shell's process group never signals allotd.
      silentLauncher().spawnv(["setsid", path]);
    },
  };
}

/** @returns {Gio.SubprocessLauncher} */
function silentLauncher() {
  const launcher = new Gio.SubprocessLauncher({
    flags: Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE,
  });
  launcher.set_cwd("/");
  return launcher;
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
