// I/O for both processes: the shell's panel button and the prefs window. Gio, GLib and Soup only,
// so the review guidelines allow it on either side.

import Gio from "gi://Gio";
import GLib from "gi://GLib";
import Soup from "gi://Soup?version=3.0";

/** @import { HttpRequest, HttpResponse } from "./settings-client.mjs" */

Gio._promisify(Soup.Session.prototype, "send_and_read_async");
Gio._promisify(Gio.File.prototype, "load_contents_async");
Gio._promisify(Gio.File.prototype, "replace_contents_bytes_async", "replace_contents_finish");

const HOME = GLib.get_home_dir();
const LOG_PATH = `${HOME}/settings-spike.log`;
const REQUEST_TIMEOUT_SECS = 6;

export const TOKEN_PATH = `${HOME}/.allot/token`;

/** The stand-in for `tray.json`: settings that belong to the panel, not to allotd. */
export const PANEL_SETTINGS_PATH = `${HOME}/.allot/settings-spike.json`;

/**
 * @typedef {object} PanelSettings
 * @property {boolean} show_cpu A stand-in panel toggle the shell shows in its menu.
 * @property {boolean} await_load Whether prefs.js waits for allotd before its window shows.
 * @property {boolean} spin_rows Numbers as `Adw.SpinRow`, else a row with a `Gtk.SpinButton`.
 */

/** @type {PanelSettings} */
const PANEL_DEFAULTS = { show_cpu: true, await_load: false, spin_rows: true };

/**
 * Appends one JSON line to `~/settings-spike.log`, which the kit and the checklist read. Blocking,
 * which only a throwaway spike may do in the shell process.
 *
 * @param {string} source `shell` or `prefs`.
 * @param {string} event
 * @param {Record<string, unknown>} [fields]
 */
export function spikeLog(source, event, fields = {}) {
  const time = GLib.DateTime.new_now_local().format("%H:%M:%S.%f")?.slice(0, 12);
  const line = `${JSON.stringify({ time, source, event, ...fields })}\n`;
  try {
    const stream = Gio.File.new_for_path(LOG_PATH).append_to(Gio.FileCreateFlags.NONE, null);
    stream.write_all(new TextEncoder().encode(line), null);
    stream.close(null);
  } catch (error) {
    console.warn(`Allot spike: cannot log: ${error}`);
  }
}

/**
 * @param {string} path
 * @returns {Promise<string | null>} `null` when the file cannot be read.
 */
export async function readTextFile(path) {
  try {
    const [contents] = await Gio.File.new_for_path(path).load_contents_async(null);
    return new TextDecoder().decode(contents);
  } catch {
    return null;
  }
}

/** @returns {Promise<PanelSettings>} */
export async function readPanelSettings() {
  const text = await readTextFile(PANEL_SETTINGS_PATH);
  try {
    return { ...PANEL_DEFAULTS, ...JSON.parse(text ?? "{}") };
  } catch {
    return { ...PANEL_DEFAULTS };
  }
}

/**
 * Replaces the file through a temporary file and a rename, as the real panel saves `tray.json`.
 *
 * @param {PanelSettings} settings
 * @returns {Promise<void>}
 */
export async function writePanelSettings(settings) {
  await Gio.File.new_for_path(PANEL_SETTINGS_PATH).replace_contents_bytes_async(
    new GLib.Bytes(new TextEncoder().encode(`${JSON.stringify(settings)}\n`)),
    null,
    false,
    Gio.FileCreateFlags.REPLACE_DESTINATION,
    null,
  );
}

/** Sends the settings client's requests with libsoup. */
export class SoupTransport {
  // allotd only listens on loopback, which a system proxy must never see.
  #session = new Soup.Session({
    proxy_resolver: Gio.SimpleProxyResolver.new(null, null),
    timeout: REQUEST_TIMEOUT_SECS,
  });

  /**
   * @param {HttpRequest} request
   * @returns {Promise<HttpResponse>}
   */
  send = async (request) => {
    const message = Soup.Message.new(request.method, request.url);
    message.request_headers.append("Authorization", `Bearer ${request.token}`);
    if (request.body !== null) {
      message.set_request_body_from_bytes(
        "application/json",
        new GLib.Bytes(new TextEncoder().encode(request.body)),
      );
    }
    const bytes = await this.#session.send_and_read_async(message, GLib.PRIORITY_DEFAULT, null);
    const body = new TextDecoder().decode(bytes.get_data() ?? new Uint8Array());
    return { status: message.get_status(), body };
  };

  close() {
    this.#session.abort();
  }
}
