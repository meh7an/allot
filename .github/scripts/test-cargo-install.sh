#!/usr/bin/env bash
# Installs Allot the way a cargo user does, from this checkout: `cargo install --path` for the
# three binary crates into a throwaway root, then `allot install`, `allot doctor` and
# `allot uninstall` from there under a throwaway home. CI only: uninstall also removes what a
# throwaway home does not cover, such as the per-user logon task on Windows.
#   test-cargo-install.sh
set -euo pipefail

if [ -z "${CI:-}" ]; then
    echo 'test-cargo-install.sh runs only on a CI runner' >&2
    exit 1
fi

work=$(mktemp -d)
failures=0
case $(uname -s) in
    MINGW* | MSYS* | CYGWIN*) windows=1 exe=.exe ;;
    *) windows='' exe='' ;;
esac

check() {
    printf 'ok: %s\n' "$1"
}

problem() {
    printf 'FAIL: %s\n' "$1" >&2
    failures=$((failures + 1))
}

native() {
    if [ -n "$windows" ]; then cygpath -m "$1"; else printf '%s' "$1"; fi
}

version=$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.name == "allot-cli") | .version' | tr -d '\r')

root="$work/cargo"
CARGO_TARGET_DIR=$(native "$PWD/target")
export CARGO_TARGET_DIR
for crate in cli daemon mcp; do
    cargo install --locked --debug --path "$crate" --root "$(native "$root")"
done
bin="$root/bin"
for binary in allot allotd allot-mcp; do
    [ -f "$bin/$binary$exe" ] || problem "cargo install wrote no $binary$exe"
done

home="$work/home"
mkdir -p "$home/.local/bin"
export HOME=$home USERPROFILE XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share"
USERPROFILE=$(native "$home")
export PATH="$bin:$PATH"

if [ "$(allot --version 2>&1)" = "allot $version" ]; then
    check "allot --version is $version"
else
    problem "allot --version said: $(allot --version 2>&1 || true)"
fi

# Beside what `allot install` needs, one binary gone: the error must say how to get it.
mv "$bin/allotd$exe" "$work/allotd-aside"
if allot install --no-claude >"$work/missing.log" 2>&1; then
    problem 'allot install passed without allotd beside it'
elif grep -q 'cargo install --locked allot-cli allotd allot-mcp' "$work/missing.log"; then
    check 'a missing binary is reported with the cargo line'
else
    problem "unexpected error for a missing binary: $(cat "$work/missing.log")"
fi
mv "$work/allotd-aside" "$bin/allotd$exe"

if [ -n "$windows" ]; then
    export ALLOT_CLAUDE="$USERPROFILE/no-claude.exe"
    install=(install --no-logon-task --launch-at-logon off)
else
    export ALLOT_CLAUDE=/nonexistent
    install=(install --no-autostart)
fi
# Into a file: on Windows the install starts allotd, which would hold a pipe open.
if allot "${install[@]}" >"$work/install.log" 2>&1; then
    check 'allot install ran from the cargo root'
else
    problem "allot install failed: $(cat "$work/install.log")"
fi
for binary in allot allotd allot-mcp; do
    [ -f "$home/.allot/bin/$binary$exe" ] || problem "allot install wrote no $binary$exe"
done
installed=$("$home/.allot/bin/allot$exe" --version 2>&1 || true)
if [ "$installed" = "allot $version" ]; then
    check "the installed allot is $version"
else
    problem "$home/.allot/bin/allot$exe --version said: $installed"
fi

# doctor exits 1 when a check fails, which the throwaway home may cause; only the path check counts.
allot doctor --json >"$work/doctor.json" || true
if jq -e '.[] | select(.name == "path")
    | .outcome == "ok" and (.detail | contains("a copy of this install"))' \
    <"$work/doctor.json" >/dev/null; then
    check "allot doctor accepts cargo's copy on PATH"
else
    problem "allot doctor's path check: $(cat "$work/doctor.json")"
fi

if allot uninstall >"$work/uninstall.log" 2>&1; then
    check 'allot uninstall ran from the cargo root'
else
    problem "allot uninstall failed: $(cat "$work/uninstall.log")"
fi

rm -rf "$work" || true
if [ "$failures" -ne 0 ]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
