// Generated from apps/gnome/allot@mehran.tech/lib/service-launcher.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
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
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __classPrivateFieldSet = (this && this.__classPrivateFieldSet) || function (receiver, state, value, kind, f) {
    if (kind === "m") throw new TypeError("Private method is not writable");
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a setter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot write private member to an object whose class did not declare it");
    return (kind === "a" ? f.call(receiver, value) : f ? f.value = value : state.set(receiver, value)), value;
};
var __classPrivateFieldGet = (this && this.__classPrivateFieldGet) || function (receiver, state, kind, f) {
    if (kind === "a" && !f) throw new TypeError("Private accessor was defined without a getter");
    if (typeof state === "function" ? receiver !== state || !f : !state.has(receiver)) throw new TypeError("Cannot read private member from an object whose class did not declare it");
    return kind === "m" ? f : kind === "a" ? f.call(receiver) : f ? f.value : state.get(receiver);
};
var _SystemdServiceLauncher_instances, _SystemdServiceLauncher_paths, _SystemdServiceLauncher_system, _SystemdServiceLauncher_unitRunsThisDaemon;
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
    /**
     * @param {LauncherPaths} paths
     * @param {ServiceSystem} system
     */
    constructor(paths, system) {
        _SystemdServiceLauncher_instances.add(this);
        _SystemdServiceLauncher_paths.set(this, void 0);
        _SystemdServiceLauncher_system.set(this, void 0);
        __classPrivateFieldSet(this, _SystemdServiceLauncher_paths, paths, "f");
        __classPrivateFieldSet(this, _SystemdServiceLauncher_system, system, "f");
    }
    /** @returns {string} */
    get daemonExe() {
        return `${__classPrivateFieldGet(this, _SystemdServiceLauncher_paths, "f").root}/bin/allotd`;
    }
    /**
     * Starts allotd without waiting for it to answer.
     *
     * @returns {Promise<string | null>} `null` when it was started, else why not.
     */
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            if (yield __classPrivateFieldGet(this, _SystemdServiceLauncher_instances, "m", _SystemdServiceLauncher_unitRunsThisDaemon).call(this)) {
                const argv = ["systemctl", "--user", "start", SYSTEMD_UNIT];
                if (yield __classPrivateFieldGet(this, _SystemdServiceLauncher_system, "f").run(argv)) {
                    return null;
                }
                // systemctl's refusals are not worth a banner line while allotd can still run directly.
            }
            const exe = this.daemonExe;
            if (!(yield __classPrivateFieldGet(this, _SystemdServiceLauncher_system, "f").isExecutable(exe))) {
                return `${exe} is missing; run allot install`;
            }
            try {
                __classPrivateFieldGet(this, _SystemdServiceLauncher_system, "f").spawn(exe);
                return null;
            }
            catch (error) {
                return `${exe}: ${error instanceof Error ? error.message : String(error)}`;
            }
        });
    }
}
_SystemdServiceLauncher_paths = new WeakMap(), _SystemdServiceLauncher_system = new WeakMap(), _SystemdServiceLauncher_instances = new WeakSet(), _SystemdServiceLauncher_unitRunsThisDaemon = function _SystemdServiceLauncher_unitRunsThisDaemon() {
    return __awaiter(this, void 0, void 0, function* () {
        const file = systemdUnitFile(__classPrivateFieldGet(this, _SystemdServiceLauncher_paths, "f").home, __classPrivateFieldGet(this, _SystemdServiceLauncher_paths, "f").configHome);
        const unit = yield __classPrivateFieldGet(this, _SystemdServiceLauncher_system, "f").readText(file);
        return unit !== null && systemdUnitExec(unit) === this.daemonExe;
    });
};
/**
 * The user unit file under `home`, in `configHome` when it is an absolute path, as the XDG spec
 * requires, and in `~/.config` otherwise. `allot_shared::autostart::systemd_unit_file`.
 *
 * @param {string} home
 * @param {string | null} configHome
 * @returns {string}
 */
export function systemdUnitFile(home, configHome) {
    const base = (configHome === null || configHome === void 0 ? void 0 : configHome.startsWith("/")) ? configHome : `${home}/.config`;
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
        }
        else {
            word += character;
        }
    }
    return null;
}
