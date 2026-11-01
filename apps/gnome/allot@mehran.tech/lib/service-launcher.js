/**
 * The system calls Start Service needs: Gio in the shell, fakes in the tests.
 *
 * @typedef {object} ServiceSystem
 * @property {(path: string) => Promise<string | null>} readText
 *   A file's text, or `null` when it cannot be read.
 * @property {(path: string) => Promise<boolean>} isExecutable
 * @property {(argv: string[]) => Promise<boolean>} run
 *   Runs a program to its end; true when it exited with 0.
 * @property {(path: string) => void} spawn
 *   Starts a program that outlives the shell's call; throws why it could not start.
 */

/**
 * @typedef {object} LauncherPaths
 * @property {string} home
 * @property {string | null} configHome
 *   The value of `XDG_CONFIG_HOME`, if set.
 * @property {string} root
 *   `~/.allot`.
 */

export const SYSTEMD_UNIT = "allotd.service";

const EXEC_START = "ExecStart=";

/**
 * Starts the installed allotd the way `allot install` set it up, as `allot-client` does: through
 * the systemd user unit when that unit starts this install's allotd, else by running
 * `~/.allot/bin/allotd` directly. The unit gives allotd its own cgroup; started directly, it stays
 * in the shell's and ends with the session.
 */
export class SystemdServiceLauncher {
  #paths;
  #system;

  /**
   * @param {LauncherPaths} paths
   * @param {ServiceSystem} system
   */
  constructor(paths, system) {
    this.#paths = paths;
    this.#system = system;
  }

  /** @returns {string} */
  get daemonExe() {
    return `${this.#paths.root}/bin/allotd`;
  }

  /**
   * Starts allotd without waiting for it to answer.
   *
   * @returns {Promise<string | null>} `null` when it was started, else why not.
   */
  async start() {
    if (await this.#unitRunsThisDaemon()) {
      const argv = ["systemctl", "--user", "start", SYSTEMD_UNIT];
      if (await this.#system.run(argv)) {
        return null;
      }
      // systemctl's refusals are not worth a banner line while allotd can still run directly.
    }
    const exe = this.daemonExe;
    if (!(await this.#system.isExecutable(exe))) {
      return `${exe} is missing; run allot install`;
    }
    try {
      this.#system.spawn(exe);
      return null;
    } catch (error) {
      return `${exe}: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  // The unit of another install (a dev build, another home) must not be started for this one.
  async #unitRunsThisDaemon() {
    const file = systemdUnitFile(this.#paths.home, this.#paths.configHome);
    const unit = await this.#system.readText(file);
    return unit !== null && systemdUnitExec(unit) === this.daemonExe;
  }
}

/**
 * The user unit file under `home`, in `configHome` when it is an absolute path, as the XDG spec
 * requires, and in `~/.config` otherwise. `allot_shared::autostart::systemd_unit_file`.
 *
 * @param {string} home
 * @param {string | null} configHome
 * @returns {string}
 */
export function systemdUnitFile(home, configHome) {
  const base = configHome?.startsWith("/") ? configHome : `${home}/.config`;
  return `${base}/systemd/user/${SYSTEMD_UNIT}`;
}

/**
 * The executable a user unit starts, from its first `ExecStart=` line, or `null` when it has
 * none. `allot_shared::autostart::systemd_unit_exec`.
 *
 * @param {string} unit
 * @returns {string | null}
 */
export function systemdUnitExec(unit) {
  for (const rawLine of unit.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line.startsWith(EXEC_START)) {
      continue;
    }
    const value = line.slice(EXEC_START.length).trim();
    const word = value.startsWith('"') ? unquoted(value.slice(1)) : value.split(/\s+/)[0];
    return word ? word.replace(/%%/g, "%").replace(/\$\$/g, "$") : null;
  }
  return null;
}

/**
 * The word up to the closing quote of a double-quoted value, with `\\` and `\"` unescaped; `null`
 * when the quote is not closed.
 *
 * @param {string} quoted
 * @returns {string | null}
 */
function unquoted(quoted) {
  let word = "";
  for (let index = 0; index < quoted.length; index++) {
    const character = quoted.charAt(index);
    if (character === '"') {
      return word;
    }
    if (character === "\\") {
      index += 1;
      word += quoted.charAt(index);
    } else {
      word += character;
    }
  }
  return null;
}
