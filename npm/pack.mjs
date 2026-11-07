// Writes and packs the npm packages: one per release target with its binaries, and the main
// package with the launcher, which depends on all of them as optional dependencies so that npm
// installs only the one matching the machine.
//   node npm/pack.mjs assets <version>
//   node npm/pack.mjs pack <version> <release dir> <out dir>
// `assets` prints the release archives the packages are made from, one per line. `pack` expects
// each of them unpacked in <release dir>, as allot-<version>-<target>/, and also writes
// packages.txt: each package's name and tarball, one per line, in the order to publish them.

import { execSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  mkdirSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const NAME = "@meh7an/allot";
const REPOSITORY = "meh7an/allot";
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const LICENSES = ["LICENSE-MIT", "LICENSE-APACHE"];
const BINARIES = ["allot", "allotd", "allot-mcp"];
const PACKAGE_LIST = "packages.txt";

// The v0.3 release targets. libc keeps a Linux without glibc from getting a binary that cannot
// start; it then gets the launcher's "not installed" error instead.
const PLATFORMS = [
  {
    target: "x86_64-pc-windows-msvc",
    archive: "zip",
    os: "win32",
    cpu: "x64",
    exe: ".exe",
    tray: true,
  },
  { target: "x86_64-unknown-linux-gnu", archive: "tar.gz", os: "linux", cpu: "x64", libc: "glibc" },
  {
    target: "aarch64-unknown-linux-gnu",
    archive: "tar.gz",
    os: "linux",
    cpu: "arm64",
    libc: "glibc",
  },
  { target: "aarch64-apple-darwin", archive: "tar.gz", os: "darwin", cpu: "arm64" },
];

const COMMON = {
  license: "MIT OR Apache-2.0",
  author: "Mehran <hi@mehran.tech>",
  homepage: `https://github.com/${REPOSITORY}#readme`,
  bugs: `https://github.com/${REPOSITORY}/issues`,
  // Trusted publishing requires this to name the publishing repository exactly.
  repository: { type: "git", url: `git+https://github.com/${REPOSITORY}.git` },
};

function platformName(platform) {
  return `${NAME}-${platform.os}-${platform.cpu}`;
}

function usage() {
  console.error("usage: node npm/pack.mjs assets <version>");
  console.error("       node npm/pack.mjs pack <version> <release dir> <out dir>");
  process.exit(2);
}

function writeJson(file, value) {
  writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function copyLicenses(dir) {
  for (const license of LICENSES) {
    cpSync(join(ROOT, license), join(dir, license));
  }
}

// Through a shell, since on Windows npm is a .cmd that only runs in one.
function pack(dir, out) {
  const output = execSync(`npm pack --json --pack-destination "${out}"`, {
    cwd: dir,
    encoding: "utf8",
  });
  const [{ filename, size, unpackedSize }] = JSON.parse(output);
  console.log(`${filename}: ${size} bytes, ${unpackedSize} unpacked`);
  return join(out, filename);
}

function writePlatformPackage(platform, version, releaseDir, dir) {
  const source = join(releaseDir, `allot-${version}-${platform.target}`);
  const exe = platform.exe ?? "";
  mkdirSync(dir, { recursive: true });
  for (const binary of BINARIES) {
    const file = join(source, `${binary}${exe}`);
    if (!statSync(file, { throwIfNoEntry: false })?.isFile()) {
      throw new Error(`${file} is missing`);
    }
    const copy = join(dir, `${binary}${exe}`);
    cpSync(file, copy);
    chmodSync(copy, 0o755);
  }
  if (platform.tray) {
    cpSync(join(source, "tray"), join(dir, "tray"), { recursive: true });
  }
  copyLicenses(dir);
  writeFileSync(
    join(dir, "README.md"),
    `# ${platformName(platform)}\n\n` +
      `The ${platform.target} binaries of Allot. Install [${NAME}]` +
      `(https://www.npmjs.com/package/${NAME}) instead; it picks this package by itself.\n`,
  );
  writeJson(join(dir, "package.json"), {
    name: platformName(platform),
    version,
    description: `The ${platform.target} binaries of Allot; install ${NAME} instead`,
    ...COMMON,
    os: [platform.os],
    cpu: [platform.cpu],
    ...(platform.libc && { libc: [platform.libc] }),
    preferUnplugged: true,
  });
}

function writeMainPackage(version, dependencies, dir) {
  mkdirSync(dir, { recursive: true });
  cpSync(join(ROOT, "npm", "bin"), join(dir, "bin"), { recursive: true });
  cpSync(join(ROOT, "npm", "README.md"), join(dir, "README.md"));
  copyLicenses(dir);
  writeJson(join(dir, "package.json"), {
    name: NAME,
    version,
    description: "Coordinates heavy builds, tests and GPU jobs between Claude Code sessions",
    keywords: ["claude-code", "mcp", "scheduling", "cpu", "gpu", "cli"],
    ...COMMON,
    bin: { allot: "bin/allot.js" },
    engines: { node: ">=18" },
    optionalDependencies: dependencies,
  });
}

function printAssets(version) {
  for (const platform of PLATFORMS) {
    console.log(`allot-${version}-${platform.target}.${platform.archive}`);
  }
}

function packAll(version, releaseDir, outDir) {
  // npm pack keeps file modes, which Windows does not have.
  if (process.platform === "win32") {
    console.warn("warning: packed on Windows, the Linux and macOS binaries are not executable");
  }
  const out = resolve(outDir);
  mkdirSync(out, { recursive: true });
  if (readdirSync(out).some((file) => file.endsWith(".tgz"))) {
    throw new Error(`${out} already holds tarballs`);
  }
  const staging = join(out, "staging");
  rmSync(staging, { recursive: true, force: true });

  const dependencies = {};
  const published = [];
  for (const platform of PLATFORMS) {
    const name = platformName(platform);
    const dir = join(staging, `${platform.os}-${platform.cpu}`);
    writePlatformPackage(platform, version, resolve(releaseDir), dir);
    const tarball = pack(dir, out);
    dependencies[name] = version;
    published.push(`${name} ${basename(tarball)}`);
  }
  const mainDir = join(staging, "main");
  writeMainPackage(version, dependencies, mainDir);
  published.push(`${NAME} ${basename(pack(mainDir, out))}`);
  rmSync(staging, { recursive: true, force: true });
  // In publishing order: a main package published first would point at missing dependencies.
  writeFileSync(join(out, PACKAGE_LIST), `${published.join("\n")}\n`);
}

function main() {
  const [command, version, releaseDir, outDir] = process.argv.slice(2);
  if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
    usage();
  }
  if (command === "assets") {
    printAssets(version);
  } else if (command === "pack" && releaseDir && outDir) {
    packAll(version, releaseDir, outDir);
  } else {
    usage();
  }
}

main();
