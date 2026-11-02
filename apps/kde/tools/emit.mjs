// Writes the widget's generated code (`pnpm build`): the KDE modules in src/ and the GNOME
// extension's logic modules, lowered by tsc to ES2016 syntax for QML's JavaScript engine, which
// has no class fields, private members or async functions; plus the generated contract module.
// Built-ins are not lowered: the typecheck holds both sets of sources to the ES2016 library.
// Private members become plain `_name` ones first: tsc would lower them to WeakMaps, and Qt 6.12's
// collector frees WeakMap values that are still in use, which takes plasmashell down.

import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const KDE_DIR = new URL("../", import.meta.url);
const APPS_DIR = new URL("../", KDE_DIR);
const REPO_DIR = new URL("../", APPS_DIR);
const GNOME_LIB = "gnome/allot@mehran.tech/lib/";
const TSC = new URL("node_modules/typescript/bin/tsc", KDE_DIR);
const CONTRACT = new URL("contract/typescript/contract.g.mjs", REPO_DIR);

export const CODE_DIR = new URL("tech.mehran.allot/contents/code/", KDE_DIR);

/** The GNOME modules the widget shares, written to `code/shared/`. */
export const SHARED_MODULES = [
  "accessible-text",
  "app-match",
  "bar-move",
  "benchmark-status",
  "client",
  "config",
  "format",
  "indicator-status",
  "job-card",
  "job-stops",
  "menu-counts",
  "notification-rules",
  "notification-text",
  "poller",
  "popup-model",
  "resources-view",
  "schedule",
  "service-banner",
  "service-launcher",
  "service-menu",
  "settings",
  "snapshot",
  "state-clock",
  "stop-result",
];

/**
 * The KDE modules and their names in `code/`. QML imports only `.mjs` files as modules; the
 * modules they import may keep `.js`.
 */
const KDE_MODULES = {
  "app-icons": "app-icons.js",
  "card-view": "card-view.mjs",
  io: "io.js",
  "notification-state": "notification-state.js",
  notifier: "notifier.js",
  rows: "rows.js",
  widget: "widget.mjs",
};

/**
 * The generated files, keyed by their path under `code/`.
 *
 * @returns {Map<string, string>}
 */
export function emit() {
  const out = mkdtempSync(join(tmpdir(), "allot-kde-"));
  try {
    const sources = [
      ...SHARED_MODULES.map((name) => `${GNOME_LIB}${name}.js`),
      ...Object.keys(KDE_MODULES).map((name) => `kde/src/${name}.js`),
    ];
    lower(sources, out);

    /** @type {Map<string, string>} */
    const files = new Map();
    files.set("contract.g.mjs", readFileSync(CONTRACT, "utf8"));
    for (const name of SHARED_MODULES) {
      const source = `${GNOME_LIB}${name}.js`;
      files.set(`shared/${name}.js`, banner(source) + readFileSync(join(out, source), "utf8"));
    }
    for (const [name, target] of Object.entries(KDE_MODULES)) {
      const source = `kde/src/${name}.js`;
      files.set(target, banner(source) + relink(readFileSync(join(out, source), "utf8")));
    }
    return files;
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}

/**
 * @param {string[]} sources Paths under `apps/`.
 * @param {string} out
 */
function lower(sources, out) {
  const input = join(out, "in");
  for (const source of sources) {
    const target = join(input, source);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, withoutPrivateNames(readFileSync(new URL(source, APPS_DIR), "utf8")));
  }
  const result = spawnSync(
    process.execPath,
    [
      fileURLToPath(TSC),
      "--ignoreConfig",
      "--allowJs",
      "--target",
      "es2016",
      "--module",
      "preserve",
      "--newLine",
      "lf",
      "--rootDir",
      input,
      "--outDir",
      out,
      ...sources.map((source) => join(input, source)),
    ],
    { encoding: "utf8" },
  );
  if (result.status !== 0) {
    throw new Error(`tsc failed:\n${result.stdout}${result.stderr}`);
  }
}

/**
 * Renames `#name` to `_name` wherever it is a member: after a dot, or starting a declaration.
 * Colours and comments such as `#N` are neither.
 *
 * @param {string} code
 * @returns {string}
 */
export function withoutPrivateNames(code) {
  return code
    .replace(/\.#([A-Za-z_]\w*)/g, "._$1")
    .replace(/^(\s*(?:(?:static|async|get|set)\s+)*)#([A-Za-z_]\w*)/gm, "$1_$2");
}

/**
 * Points the KDE modules' imports of the GNOME modules at their copies next to them.
 *
 * @param {string} code
 * @returns {string}
 */
function relink(code) {
  return code
    .replaceAll(`"../../${GNOME_LIB}`, `"./shared/`)
    .replaceAll(`"../../gnome/allot@mehran.tech/contract.g.mjs"`, `"./contract.g.mjs"`);
}

/**
 * @param {string} source
 * @returns {string}
 */
function banner(source) {
  return `// Generated from apps/${source} by apps/kde/tools/emit.mjs (pnpm build); do not edit.\n`;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  rmSync(CODE_DIR, { recursive: true, force: true });
  for (const [path, contents] of emit()) {
    const target = new URL(path, CODE_DIR);
    mkdirSync(new URL("./", target), { recursive: true });
    writeFileSync(target, contents);
    console.log(fileURLToPath(target));
  }
}
