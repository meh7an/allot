#!/usr/bin/env bash
# Tests install.sh: refusals on unsupported platforms and versions (with a fake uname, getconf
# and sw_vers), the checksum check and argument passing against a local server with a stub
# package, then a real install of the latest release into a throwaway HOME, which --offline
# leaves out (for a machine with a real install).
#   test-install-sh.sh <path to install.sh> [--offline]
set -euo pipefail

script=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
work=$(mktemp -d)
trap 'kill "${server:-}" 2>/dev/null || true; rm -rf "$work"' EXIT
failures=0

check() {
    printf 'ok: %s\n' "$1"
}

problem() {
    printf 'FAIL: %s\n' "$1" >&2
    failures=$((failures + 1))
}

# Runs install.sh with the given environment assignments and arguments, and checks that it
# fails with a message containing $1. Downloads would go nowhere, so a refusal that came too
# late shows up as a download error instead.
expect_refusal() {
    local want=$1 output
    shift
    if output=$(env ALLOT_DOWNLOAD_BASE=http://127.0.0.1:9 "$@" sh "$script" 2>&1); then
        problem "succeeded, expected '$want'"
    elif [[ $output != *"$want"* ]]; then
        problem "expected '$want', got: $output"
    else
        check "$want"
    fi
}

sha256() {
    if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

# A directory of fake platform commands, put first on PATH.
fake_platform() {
    local dir=$work/fake-$1-$2
    mkdir -p "$dir"
    printf '#!/bin/sh\ncase $1 in -s) echo %s ;; -m) echo %s ;; esac\n' "$1" "$2" >"$dir/uname"
    printf '#!/bin/sh\n%s\n' "${3:-exit 1}" >"$dir/getconf"
    printf '#!/bin/sh\n%s\n' "${4:-exit 1}" >"$dir/sw_vers"
    printf '#!/bin/sh\nexit 1\n' >"$dir/sysctl"
    chmod +x "$dir"/*
    printf '%s:%s' "$dir" "$PATH"
}

echo '--- unsupported platforms and versions'
expect_refusal 'FreeBSD is not supported' PATH="$(fake_platform FreeBSD amd64)"
expect_refusal 'Linux on i686 is not supported' PATH="$(fake_platform Linux i686)"
expect_refusal 'this system has none (musl?)' PATH="$(fake_platform Linux x86_64)"
expect_refusal 'and this has 2.31' \
    PATH="$(fake_platform Linux x86_64 'echo glibc 2.31')"
expect_refusal 'macOS on x86_64 is not supported' PATH="$(fake_platform Darwin x86_64)"
expect_refusal 'needs macOS 14 or newer, and this is 13.6' \
    PATH="$(fake_platform Darwin arm64 '' 'echo 13.6')"
expect_refusal 'run this in PowerShell instead' PATH="$(fake_platform MINGW64_NT-10.0 x86_64)"
expect_refusal 'Allot 0.2.0 has no Linux or macOS package' ALLOT_VERSION=0.2.0
expect_refusal "'latest' is not a release version" ALLOT_VERSION=latest

echo '--- checksums and arguments, against a local server'
release=$work/release
download=$release/download/v0.3.0
mkdir -p "$download"
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu aarch64-apple-darwin; do
    package=allot-0.3.0-$target
    mkdir -p "$work/stub/$package"
    printf '#!/bin/sh\nprintf "%%s\\n" "$*" >"$STUB_MARKER"\n' >"$work/stub/$package/allot"
    chmod +x "$work/stub/$package/allot"
    tar -czf "$download/$package.tar.gz" -C "$work/stub" "$package"
    (cd "$download" && sha256 "$package.tar.gz" >"$package.tar.gz.sha256")
done
port=47931
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$release" >/dev/null 2>&1 &
server=$!
for _ in $(seq 50); do
    curl -fs -o /dev/null "http://127.0.0.1:$port/" && break
    sleep 0.1
done
local_base=http://127.0.0.1:$port
marker=$work/marker

run_local() {
    rm -f "$marker"
    env HOME="$work/stub-home" STUB_MARKER="$marker" ALLOT_DOWNLOAD_BASE="$local_base" \
        sh "$script" "$@"
}

if output=$(run_local --version 0.3.0 --no-claude --no-autostart 2>&1) &&
    [[ $(cat "$marker" 2>/dev/null) == 'install --no-claude --no-autostart' ]]; then
    check 'a good package runs allot install with the other arguments'
else
    problem "the stub did not get 'install --no-claude --no-autostart': $output"
fi
if ALLOT_VERSION=v0.3.0 run_local >/dev/null 2>&1 && [[ $(cat "$marker") == install ]]; then
    check 'ALLOT_VERSION pins the release, with or without the v'
else
    problem 'ALLOT_VERSION=v0.3.0 did not install the stub'
fi

for file in "$download"/*.sha256; do
    asset=$(basename "$file" .sha256)
    printf '%064d  %s\n' 0 "$asset" >"$file"
done
if output=$(run_local --version 0.3.0 2>&1); then
    problem 'a checksum mismatch was installed'
elif [[ $output != *'checksum mismatch for allot-0.3.0-'* ]]; then
    problem "a mismatch failed for another reason: $output"
elif [[ -e $marker ]]; then
    problem 'the package ran after a checksum mismatch'
else
    check 'a checksum mismatch stops before anything runs'
fi
for file in "$download"/*.sha256; do
    printf '%064d  other.tar.gz\n' 0 >"$file"
done
if output=$(run_local --version 0.3.0 2>&1) || [[ $output != *'does not hold a checksum'* ]]; then
    problem "a .sha256 for another file was not refused: $output"
else
    check 'a .sha256 naming another file is refused'
fi

real_install() {
    local home=$work/home latest installed
    mkdir -p "$home/.local/bin"
    env HOME="$home" XDG_CONFIG_HOME="$home/.config" XDG_DATA_HOME="$home/.local/share" \
        ALLOT_CLAUDE=/nonexistent sh -s -- --no-autostart <"$script"
    latest=$(curl -fsSL -o /dev/null -w '%{url_effective}' \
        https://github.com/meh7an/allot/releases/latest)
    latest=${latest##*/v}
    installed=$("$home/.local/bin/allot" --version)
    if [[ $installed == "allot $latest" ]]; then
        check "installed $installed and linked it into ~/.local/bin"
    else
        problem "expected allot $latest, got $installed"
    fi
    if grep -q 'allot' "$home/.claude/settings.json" && [[ -f $home/.claude/allot.md ]]; then
        check 'set up Claude Code in the throwaway HOME'
    else
        problem 'no hooks or rule in the throwaway HOME'
    fi
}

if [[ ${2:-} == --offline ]]; then
    echo '--- skipped the real install (--offline)'
else
    echo '--- the latest release, into a throwaway HOME'
    real_install
fi

if ((failures)); then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo 'all checks passed'
