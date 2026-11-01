// Generated from apps/gnome/allot@mehran.tech/lib/app-match.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
// Which installed app a job's process belongs to, so its tile shows the app's icon. Command-line
// tools have no desktop entry and keep their initials.
/**
 * An installed app as the shell lists it.
 *
 * @typedef {object} DesktopApp
 * @property {string} id
 *   The desktop file ID, for example `code.desktop`.
 * @property {string} exec
 *   The `Exec` line, or empty.
 * @property {string} tryExec
 *   The `TryExec` value, or empty.
 */
// Launchers and interpreters run every kind of program, so an app whose Exec starts with one
// says nothing about other processes of the same name. Versioned ones (`python3.13`) count too.
const GENERIC_PROGRAM = /^(bash|dash|env|fish|flatpak|gjs|java|lua|node|perl|php|python|ruby|sh|snap|zsh)[\d.]*$/;
const FLATPAK_APP = /\/flatpak\/app\/([^/]+)\//;
const SNAP_APP = /^\/snap\/([^/]+)\//;
/**
 * The desktop file ID a sandboxed app's executable path names: Flatpak installs under
 * `.../flatpak/app/<id>/`, Snap under `/snap/<name>/`.
 *
 * @param {string} exe
 * @returns {string | null}
 */
export function sandboxedAppId(exe) {
    const flatpak = FLATPAK_APP.exec(exe);
    if (flatpak !== null) {
        return `${flatpak[1]}.desktop`;
    }
    const snap = SNAP_APP.exec(exe);
    if (snap !== null) {
        return `${snap[1]}_${snap[1]}.desktop`;
    }
    return null;
}
/**
 * The program an `Exec` line starts, without `env` and its variable assignments; empty when
 * there is none.
 *
 * @param {string} exec
 * @returns {string}
 */
export function execProgram(exec) {
    var _a, _b, _c;
    const words = exec.trim().split(/\s+/).map(unquote);
    let index = 0;
    if (basename((_a = words[0]) !== null && _a !== void 0 ? _a : "") === "env") {
        index = 1;
        while (((_b = words[index]) !== null && _b !== void 0 ? _b : "").includes("=")) {
            index += 1;
        }
    }
    return (_c = words[index]) !== null && _c !== void 0 ? _c : "";
}
/**
 * The app whose `TryExec` or `Exec` program is the executable, by full path or, for a program
 * found on `PATH`, by name. The first such app wins; generic launchers never match.
 *
 * @param {string} exe The process's executable path.
 * @param {readonly DesktopApp[]} apps
 * @returns {string | null} The app's desktop file ID.
 */
export function matchingApp(exe, apps) {
    const name = basename(exe);
    if (name === "" || GENERIC_PROGRAM.test(name)) {
        return null;
    }
    for (const app of apps) {
        for (const program of [app.tryExec.trim(), execProgram(app.exec)]) {
            if (program === "") {
                continue;
            }
            const matches = program.startsWith("/") ? program === exe : program === name;
            if (matches) {
                return app.id;
            }
        }
    }
    return null;
}
/**
 * @param {string} path
 * @returns {string}
 */
function basename(path) {
    return path.slice(path.lastIndexOf("/") + 1);
}
/**
 * @param {string} word
 * @returns {string}
 */
function unquote(word) {
    return word.replace(/^["']|["']$/g, "");
}
