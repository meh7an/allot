import Shell from "gi://Shell";

import { matchingApp, sandboxedAppId } from "./lib/app-match.js";

/** @import { DesktopApp } from "./lib/app-match.js" */

/**
 * The icon of the installed app a process belongs to, looked up once per executable. A
 * command-line tool has no app, so its tile keeps the initials.
 */
export class AppIcons {
  #system = Shell.AppSystem.get_default();
  /** @type {Map<string, Gio.Icon | null>} */
  #cache = new Map();
  /** @type {DesktopApp[] | null} */
  #apps = null;
  #installedChangedId;

  constructor() {
    this.#installedChangedId = this.#system.connect("installed-changed", () => {
      this.#apps = null;
      this.#cache.clear();
    });
  }

  /**
   * @param {string | null} exe
   * @returns {Gio.Icon | null}
   */
  icon = (exe) => {
    if (exe === null || !exe.startsWith("/")) {
      return null;
    }
    const cached = this.#cache.get(exe);
    if (cached !== undefined) {
      return cached;
    }
    const id = sandboxedAppId(exe) ?? matchingApp(exe, this.#installed());
    const icon = (id === null ? null : this.#system.lookup_app(id)?.get_icon()) ?? null;
    this.#cache.set(exe, icon);
    return icon;
  };

  destroy() {
    this.#system.disconnect(this.#installedChangedId);
  }

  /** @returns {DesktopApp[]} */
  #installed() {
    this.#apps ??= this.#system
      .get_installed()
      .filter((info) => info.should_show())
      .map((info) => ({
        id: info.get_id() ?? "",
        exec: info.get_commandline() ?? "",
        // Not every AppInfo is a desktop entry, and naming Gio.DesktopAppInfo makes GJS warn.
        tryExec: info.get_string?.("TryExec") ?? "",
      }));
    return this.#apps;
  }
}
