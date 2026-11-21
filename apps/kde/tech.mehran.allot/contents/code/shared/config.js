// Generated from apps/gnome/allot@mehran.tech/lib/config.js by apps/kde/tools/emit.mjs (pnpm build); do not edit.
/**
 * @typedef {object} DataPaths
 * @property {string} root
 *   `~/.allot`, where allotd keeps everything.
 * @property {string} config
 *   The daemon config. Optional; allotd runs on defaults without it.
 * @property {string} logs
 * @property {string} settings
 *   `tray.json`, what the popup remembers, in the other apps' format.
 */
/**
 * The per-user files under `~/.allot` that panels use, as in `allot_shared::Paths`.
 *
 * @param {string} home
 * @returns {DataPaths}
 */
export function dataPaths(home) {
    const root = `${home.replace(/\/+$/, "")}/.allot`;
    return {
        root,
        config: `${root}/config.toml`,
        logs: `${root}/logs`,
        settings: `${root}/tray.json`,
    };
}
