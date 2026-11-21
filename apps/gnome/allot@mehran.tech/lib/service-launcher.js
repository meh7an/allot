/**
 * What a finished program left: whether it exited with 0, and what it printed (standard output
 * and error together).
 *
 * @typedef {object} RunResult
 * @property {boolean} ok
 * @property {string} output
 */

/**
 * The system calls Start Service needs: Gio in the shell, fakes in the tests.
 *
 * @typedef {object} ServiceSystem
 * @property {(path: string) => Promise<string | null>} readText
 *   A file's text, or `null` when it cannot be read.
 * @property {(argv: string[]) => Promise<RunResult>} run
 *   Runs a program to its end.
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

export const NO_UNIT_TEXT = `${SYSTEMD_UNIT} is not installed; run allot install`;

export const RUNS_OFF_BUS_TEXT =
  "allotd already runs without the session bus; restart it outside the sandbox";

const EXEC_START = "ExecStart=";

// What systemctl prints when the unit is unknown, missing or masked, in the wording of the
// systemd versions that ship with the supported distributions.
const NO_UNIT_OUTPUT = /not found|not-found|not loaded|could not be found|does not exist|masked/i;

/**
 * Starts the installed allotd the way `allot install` set it up: through the systemd user unit,
 * which gives allotd its own cgroup. It never runs allotd itself, so a second allotd cannot
 * appear beside one the unit already manages.
 */
export class SystemdServiceLauncher {
  #paths;
  #system;
  #aliveOffBus;

  /**
   * @param {LauncherPaths} paths
   * @param {ServiceSystem} system
   * @param {(() => Promise<boolean>) | null} [aliveOffBus] Whether an allotd process runs
   *   without the panel's channel to it; Start then leaves it alone.
   */
  constructor(paths, system, aliveOffBus = null) {
    this.#paths = paths;
    this.#system = system;
    this.#aliveOffBus = aliveOffBus;
  }

  /** @returns {string} */
  get daemonExe() {
    return `${this.#paths.root}/bin/allotd`;
  }

  /**
   * Starts allotd through the unit without waiting for it to answer.
   *
   * @returns {Promise<string | null>} `null` when it was started, else why not.
   */
  async start() {
    if ((await this.#aliveOffBus?.()) === true) {
      return RUNS_OFF_BUS_TEXT;
    }
    return this.#systemctl("start");
  }

  /**
   * Restarts allotd through the unit, whatever it runs, for an allotd the panel cannot ask to
   * stop.
   *
   * @returns {Promise<string | null>} `null` when systemd restarted it, else why not.
   */
  restartUnit() {
    return this.#systemctl("restart");
  }

  /**
   * Restarts allotd through the systemd user unit, when that unit runs this install's allotd.
   * Asks allotd for nothing, so it also mends a refused key, and writes no stop record.
   *
   * @returns {Promise<boolean>} Whether systemd restarted it; false leaves allotd as it was.
   */
  async restartThroughUnit() {
    if (!(await this.#unitRunsThisDaemon())) {
      return false;
    }
    return (await this.restartUnit()) === null;
  }

  /**
   * @param {"start" | "restart"} verb
   * @returns {Promise<string | null>}
   */
  async #systemctl(verb) {
    const result = await this.#system.run(["systemctl", "--user", verb, SYSTEMD_UNIT]);
    if (result.ok) {
      return null;
    }
    if (NO_UNIT_OUTPUT.test(result.output)) {
      return NO_UNIT_TEXT;
    }
    const reason = lastLine(result.output);
    return reason === "" ? `systemctl --user ${verb} ${SYSTEMD_UNIT} failed` : reason;
  }

  // The unit of another install (a dev build, another home) must not be restarted for this one.
  async #unitRunsThisDaemon() {
    const file = systemdUnitFile(this.#paths.home, this.#paths.configHome);
    const unit = await this.#system.readText(file);
    return unit !== null && systemdUnitExec(unit) === this.daemonExe;
  }
}

/**
 * @param {string} text
 * @returns {string} The last line with text in it, trimmed.
 */
function lastLine(text) {
  const lines = text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== "");
  return lines[lines.length - 1] ?? "";
}

/** How long a restart waits for the old allotd to stop answering. */
export const RESTART_WAIT_MS = 10_000;

const RESTART_POLL_MS = 250;

/**
 * What Restart Service needs from the client: GNOME's `BusClient`, the widget's client, or
 * a fake in tests.
 *
 * @typedef {object} RestartClient
 * @property {(options: { record: boolean }) => Promise<string | null>} shutdown
 * @property {() => Promise<string | null>} healthVersion
 */

/**
 * What Restart Service needs from the launcher.
 *
 * @typedef {object} RestartLauncher
 * @property {() => Promise<boolean>} restartThroughUnit
 * @property {() => Promise<string | null>} restartUnit
 * @property {() => Promise<string | null>} start
 */

/**
 * Restarts allotd, for a new port or a refused key: through the systemd unit when it runs this
 * allotd, else a shutdown with no stop record, a wait of up to 10 s for allotd to stop
 * answering, and a start.
 *
 * @param {RestartClient} client
 * @param {RestartLauncher} launcher
 * @param {import("./poller.js").Timers} timers
 * @returns {Promise<string | null>} `null` when allotd was started again, else why not.
 */
export async function restartService(client, launcher, timers) {
  if (await launcher.restartThroughUnit()) {
    return null;
  }
  const problem = await client.shutdown({ record: false });
  if (problem !== null) {
    return problem;
  }
  for (let waited = 0; waited < RESTART_WAIT_MS; waited += RESTART_POLL_MS) {
    if ((await client.healthVersion()) === null) {
      break;
    }
    await new Promise((resolve) => timers.setTimeout(() => resolve(undefined), RESTART_POLL_MS));
  }
  return launcher.start();
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
