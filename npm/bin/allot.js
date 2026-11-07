#!/usr/bin/env node
// Runs allot from the platform package npm installed beside this one. A launcher rather than an
// install script, so it also works where scripts do not run: pnpm and bun by default, and
// --ignore-scripts everywhere.
"use strict";

const { spawn } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { name, version, optionalDependencies } = require("../package.json");

const PLATFORM_PACKAGE = `${name}-${process.platform}-${process.arch}`;
const EXE = process.platform === "win32" ? "allot.exe" : "allot";
// These set up ~/.allot/bin themselves, so a hint to run `allot install` before them is noise.
const SETUP_COMMANDS = ["install", "uninstall"];

function fail(message, hint) {
  console.error(`Error: ${message}`);
  if (hint) {
    console.error(`hint: ${hint}`);
  }
  process.exit(1);
}

function findBinary() {
  if (!(PLATFORM_PACKAGE in optionalDependencies)) {
    const supported = Object.keys(optionalDependencies)
      .map((dependency) => dependency.slice(name.length + 1))
      .join(", ");
    fail(
      `Allot has no build for ${process.platform}-${process.arch}`,
      `it runs on ${supported}`,
    );
  }
  try {
    return path.join(path.dirname(require.resolve(`${PLATFORM_PACKAGE}/package.json`)), EXE);
  } catch {
    fail(
      `${PLATFORM_PACKAGE} is not installed`,
      `npm leaves it out with --omit=optional, and on a Linux without glibc; ` +
        `reinstall with npm install -g ${name}`,
    );
  }
}

// npm only fills node_modules; allotd, the MCP server and the hooks run from ~/.allot/bin, which
// `allot install` fills. A size check rather than running that copy keeps every command fast, and
// two builds of one version have the same size.
function printSetupHint(binary, args) {
  if (!process.stderr.isTTY || args.some((arg) => SETUP_COMMANDS.includes(arg))) {
    return;
  }
  const installed = path.join(os.homedir(), ".allot", "bin", EXE);
  let installedSize;
  try {
    installedSize = fs.statSync(installed).size;
  } catch {
    console.error("note: Allot is not set up yet; run `allot install` to finish");
    return;
  }
  if (installedSize !== fs.statSync(binary).size) {
    console.error(
      `note: ${installed} is not npm's allot ${version}; run \`allot install\` to update it`,
    );
  }
}

function run(binary, args) {
  const child = spawn(binary, args, {
    stdio: "inherit",
    env: { ...process.env, ALLOT_NPM_LAUNCHER: __filename },
  });
  // Ctrl-C reaches allot from the terminal by itself; this process only waits for it to exit.
  process.on("SIGINT", () => {});
  for (const signal of ["SIGTERM", "SIGHUP"]) {
    process.on(signal, () => child.kill(signal));
  }
  child.on("error", (error) => fail(`could not run ${binary}: ${error.message}`));
  child.on("exit", (code, signal) => {
    if (signal) {
      process.removeAllListeners(signal);
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
}

const args = process.argv.slice(2);
const binary = findBinary();
printSetupHint(binary, args);
run(binary, args);
