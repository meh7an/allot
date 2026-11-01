import { matchingApp, sandboxedAppId } from "../../gnome/allot@mehran.tech/lib/app-match.js";

/** @import { DesktopApp } from "../../gnome/allot@mehran.tech/lib/app-match.js" */
/** @import { CommandRunner } from "./io.js" */

/**
 * An installed app with the icon its desktop entry names.
 *
 * @typedef {DesktopApp & { icon: string }} InstalledApp
 */

/**
 * Prints the keys that matter of every desktop entry, one `./<path>.desktop:<key>=<value>` per
 * line, with `--` before each data directory. Directories come in XDG order, so an entry in an
 * earlier one hides the same ID in a later one. The list goes through a variable because a shell
 * splits only what an expansion produced, never a `:` written between two expansions.
 */
export const DESKTOP_ENTRIES_COMMAND =
  "dirs=${XDG_DATA_HOME:-$HOME/.local/share}:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}; " +
  "IFS=:; for d in $dirs; " +
  'do echo --; (cd "$d/applications" 2>/dev/null && ' +
  "find . -name '*.desktop' -exec grep -HE '^(Type|NoDisplay|Hidden|Exec|TryExec|Icon)=' {} +); " +
  "done";

const ENTRY_LINE = /^\.\/(.+\.desktop):(\w+)=(.*)$/;
const DIRECTORY_MARK = "--";

/**
 * The apps a user can launch, from the output of `DESKTOP_ENTRIES_COMMAND`. A desktop file ID is
 * its path under `applications` with `-` for `/`. An action's `Exec` comes after the entry's own,
 * so the first value of a key is the entry's.
 *
 * @param {string} output
 * @returns {InstalledApp[]}
 */
export function parseDesktopEntries(output) {
  /** @type {Map<string, Map<string, string>>} */
  const entries = new Map();
  /** @type {Set<string>} */
  let current = new Set();
  /** @type {Set<string>} */
  const earlier = new Set();
  for (const line of output.split("\n")) {
    if (line === DIRECTORY_MARK) {
      current.forEach((id) => earlier.add(id));
      current = new Set();
      continue;
    }
    const match = ENTRY_LINE.exec(line);
    if (match === null) {
      continue;
    }
    const [, path = "", key = "", value = ""] = match;
    const id = path.replace(/\//g, "-");
    if (earlier.has(id)) {
      continue;
    }
    current.add(id);
    const entry = entries.get(id) ?? new Map();
    entries.set(id, entry);
    if (!entry.has(key)) {
      entry.set(key, value);
    }
  }

  /** @type {InstalledApp[]} */
  const apps = [];
  entries.forEach((entry, id) => {
    const launchable =
      entry.get("Type") === "Application" &&
      entry.get("NoDisplay") !== "true" &&
      entry.get("Hidden") !== "true";
    if (launchable) {
      apps.push({
        id,
        exec: entry.get("Exec") ?? "",
        tryExec: entry.get("TryExec") ?? "",
        icon: entry.get("Icon") ?? "",
      });
    }
  });
  return apps;
}

/**
 * What `Kirigami.Icon` takes for a desktop entry's `Icon`: a theme name as it is, a path as a URL.
 *
 * @param {string} icon
 * @returns {string | null}
 */
export function iconSource(icon) {
  if (icon === "") {
    return null;
  }
  return icon.startsWith("/") ? `file://${icon}` : icon;
}

/**
 * The icon of the installed app a process belongs to. A command-line tool has no app, so its tile
 * keeps the initials. The entries are read when the first card asks and forgotten when the popup
 * closes, which picks up apps installed in between.
 */
export class AppIcons {
  #runner;
  #onLoaded;
  /** @type {InstalledApp[] | null} */
  #apps = null;
  /** @type {Map<string, string | null>} */
  #cache = new Map();
  #isLoading = false;

  /**
   * @param {CommandRunner} runner
   * @param {() => void} onLoaded Called once the entries are read, so the cards can show them.
   */
  constructor(runner, onLoaded) {
    this.#runner = runner;
    this.#onLoaded = onLoaded;
  }

  /**
   * @param {string | null} exe
   * @returns {string | null} The icon's source, or `null` for none or while the entries are read.
   */
  icon = (exe) => {
    if (exe === null || !exe.startsWith("/")) {
      return null;
    }
    if (this.#apps === null) {
      this.#load();
      return null;
    }
    const cached = this.#cache.get(exe);
    if (cached !== undefined) {
      return cached;
    }
    const apps = this.#apps;
    const id = sandboxedAppId(exe) ?? matchingApp(exe, apps);
    const app = apps.find((candidate) => candidate.id === id);
    const icon = app === undefined ? null : iconSource(app.icon);
    this.#cache.set(exe, icon);
    return icon;
  };

  forget() {
    this.#apps = null;
    this.#cache.clear();
  }

  async #load() {
    if (this.#isLoading) {
      return;
    }
    this.#isLoading = true;
    const output = await this.#runner.run(DESKTOP_ENTRIES_COMMAND);
    this.#isLoading = false;
    this.#apps = parseDesktopEntries(output.stdout);
    this.#cache.clear();
    this.#onLoaded();
  }
}
