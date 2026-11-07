#!/usr/bin/env bash
# Tests the npm packages that npm-packages.sh packed into <dir>: installs the main package with
# npm into a throwaway prefix and home, with its optional dependencies pointed at the tarballs
# beside it (the registry may not have them yet), and checks that npm picked only this machine's
# binaries, that `allot --version` runs through the launcher, that the launcher's setup hint comes
# and goes, and that `allot install`, `allot doctor` and `allot uninstall` work from it.
#   test-npm.sh <dir>
set -euo pipefail

packages=$(cd "$1" && pwd)
work=$(mktemp -d)
failures=0
case $(uname -s) in
    MINGW* | MSYS* | CYGWIN*) windows=1 ;;
    *) windows= ;;
esac

check() {
    printf 'ok: %s\n' "$1"
}

problem() {
    printf 'FAIL: %s\n' "$1" >&2
    failures=$((failures + 1))
}

# Node takes Windows paths; Git Bash's /d/a/... forms mean nothing to it.
native() {
    if [ -n "$windows" ]; then cygpath -m "$1"; else printf '%s' "$1"; fi
}

main_tarball=$(awk 'END { print $2 }' "$packages/packages.txt")
version=$(printf '%s' "$main_tarball" | sed -E 's/^meh7an-allot-(.*)\.tgz$/\1/')

# The main package as published names its platform packages by version.
mkdir "$work/main"
tar -xzf "$packages/$main_tarball" -C "$work/main"
manifest=$(native "$work/main/package/package.json")
if node -e '
    const { version, optionalDependencies } = require(process.argv[1]);
    const values = Object.values(optionalDependencies);
    process.exit(values.length === 4 && values.every((value) => value === version) ? 0 : 1);
' "$manifest"; then
    check 'the main package depends on the four platform packages at its own version'
else
    problem "unexpected optionalDependencies in $main_tarball"
fi

# The same package, pointed at the local tarballs and packed again.
# shellcheck disable=SC2016 # JavaScript template literals, not shell expansions.
node -e '
    const fs = require("node:fs");
    const [file, dir] = process.argv.slice(1);
    const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
    const lines = fs.readFileSync(`${dir}/packages.txt`, "utf8").trim().split("\n");
    for (const [name, tarball] of lines.map((line) => line.split(" "))) {
        if (name in manifest.optionalDependencies) {
            manifest.optionalDependencies[name] = `file:${dir}/${tarball}`;
        }
    }
    fs.writeFileSync(file, `${JSON.stringify(manifest, null, 2)}\n`);
' "$manifest" "$(native "$packages")"
mkdir "$work/local"
(cd "$work/main/package" && npm pack --pack-destination "$(native "$work/local")" >/dev/null)

prefix="$work/prefix"
home="$work/home"
mkdir -p "$home"
export HOME=$home USERPROFILE
USERPROFILE=$(native "$home")
export npm_config_cache="$work/cache" npm_config_update_notifier=false npm_config_fund=false
npm install --global --prefix "$(native "$prefix")" "$(native "$work/local/$main_tarball")"
if [ -n "$windows" ]; then
    bin=$prefix
    modules=$prefix/node_modules
else
    bin=$prefix/bin
    modules=$prefix/lib/node_modules
fi
export PATH="$bin:$PATH"

expected="$(node -p 'process.platform')-$(node -p 'process.arch')"
found=$(find "$modules" -type d -path '*/@meh7an/allot-*' -prune | sed 's#.*/allot-##' | sort)
if [ "$found" = "$expected" ]; then
    check "npm installed only allot-$expected"
else
    problem "expected only allot-$expected, found: $(echo "$found" | tr '\n' ' ')"
fi

if [ "$(allot --version 2>&1)" = "allot $version" ]; then
    check "allot --version is $version through the launcher"
else
    problem "allot --version said: $(allot --version 2>&1 || true)"
fi
if [ -n "$windows" ]; then
    if [ "$(cmd //d //c allot.cmd --version 2>&1 | tr -d '\r')" = "allot $version" ]; then
        check 'allot.cmd runs too'
    else
        problem "allot.cmd --version said: $(cmd //d //c allot.cmd --version 2>&1 || true)"
    fi
fi

# The hint is only for a terminal, which `script` provides on Linux.
hint() {
    script -qec 'allot --version' /dev/null 2>&1 | tr -d '\r'
}
if [ "$(uname -s)" = Linux ]; then
    if [[ $(hint) == *'not set up yet'* ]]; then
        check 'in a terminal the launcher asks for allot install before it has run'
    else
        problem "no setup hint before allot install: $(hint)"
    fi
fi

if [ -n "$windows" ]; then
    export ALLOT_CLAUDE="$USERPROFILE/no-claude.exe"
    install=(install --no-logon-task --launch-at-logon off)
else
    export ALLOT_CLAUDE=/nonexistent
    install=(install --no-autostart)
fi
# Into a file: on Windows the install starts allotd and the tray app, which would hold a pipe open.
if allot "${install[@]}" >"$work/install.log" 2>&1; then
    check 'allot install ran from the npm package'
else
    problem "allot install failed: $(cat "$work/install.log")"
fi
exe=
[ -z "$windows" ] || exe=.exe
for binary in allot allotd allot-mcp; do
    [ -f "$home/.allot/bin/$binary$exe" ] || problem "allot install wrote no $binary$exe"
done
installed=$("$home/.allot/bin/allot$exe" --version 2>&1 || true)
if [ "$installed" = "allot $version" ]; then
    check "~/.allot/bin/allot is $version"
else
    problem "~/.allot/bin/allot --version said: $installed"
fi
if [ "$(uname -s)" = Linux ]; then
    if [ "$(hint)" = "allot $version" ]; then
        check 'the setup hint is gone after allot install'
    else
        problem "a hint is still printed after allot install: $(hint)"
    fi
fi

# doctor exits 1 when a check fails, which the throwaway home may cause; only the path check counts.
# Releases before 0.4.0 have no doctor.
doctor_accepts_launcher() {
    allot doctor --json >"$work/doctor.json" || true
    node -e '
        const checks = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8"));
        const path = checks.find((check) => check.name === "path");
        process.exit(path && path.outcome === "ok" && path.detail.includes("npm") ? 0 : 1);
    ' "$(native "$work/doctor.json")"
}
if ! allot doctor --help >/dev/null 2>&1; then
    printf 'skip: allot %s has no doctor\n' "$version"
elif doctor_accepts_launcher; then
    check "allot doctor accepts npm's launcher on PATH"
else
    problem "allot doctor's path check: $(cat "$work/doctor.json")"
fi

if allot uninstall >"$work/uninstall.log" 2>&1; then
    check 'allot uninstall ran from the npm package'
else
    problem "allot uninstall failed: $(cat "$work/uninstall.log")"
fi

rm -rf "$work" || true
if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
