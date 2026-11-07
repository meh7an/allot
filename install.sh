#!/bin/sh
# Installs Allot on Linux or macOS from a GitHub release: picks the package for this machine,
# checks it against its .sha256, and runs its `allot install`, which does the real setup.
#
#   curl -fsSL https://raw.githubusercontent.com/meh7an/allot/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/meh7an/allot/main/install.sh | sh -s -- \
#     --version 0.3.0 --no-claude
#
# --version X.Y.Z (or ALLOT_VERSION) installs that release instead of the latest; every other
# argument goes to `allot install` (see `allot install --help`). ALLOT_DOWNLOAD_BASE replaces
# https://github.com/meh7an/allot/releases, for a mirror or a test.
#
# Everything runs from main, at the end, so a download cut off halfway runs nothing.

set -eu

# The first release with Linux and macOS packages.
FIRST_VERSION=0.3.0
GLIBC_MIN=2.35
MACOS_MIN=14

say() {
    printf 'allot-install: %s\n' "$*"
}

fail() {
    printf 'allot-install: error: %s\n' "$*" >&2
    exit 1
}

need() {
    command -v "$1" >/dev/null 2>&1 || fail "this script needs $1, which is not on PATH"
}

usage() {
    cat <<'EOF'
Installs Allot from a GitHub release and runs `allot install`.

Usage: install.sh [--version X.Y.Z] [allot install options]

  --version X.Y.Z   Install this release instead of the latest (or set ALLOT_VERSION)
  -h, --help        Show this help

Other options go to `allot install`, such as --no-claude and --no-autostart.
EOF
}

# True when the dotted version $1 is at least $2.
at_least() {
    awk -v have="$1" -v want="$2" 'BEGIN {
        n = split(have, h, "."); m = split(want, w, ".")
        for (i = 1; i <= (n > m ? n : m); i++) {
            if (h[i] + 0 > w[i] + 0) exit 0
            if (h[i] + 0 < w[i] + 0) exit 1
        }
        exit 0
    }'
}

check_glibc() {
    libc=$(getconf GNU_LIBC_VERSION 2>/dev/null || true)
    case $libc in
    "glibc "*) ;;
    *) fail "Allot needs glibc $GLIBC_MIN or newer, and this system has none (musl?)" ;;
    esac
    have=${libc#glibc }
    at_least "$have" "$GLIBC_MIN" ||
        fail "Allot needs glibc $GLIBC_MIN or newer (Ubuntu 22.04, Debian 12), and this has $have"
}

check_macos() {
    have=$(sw_vers -productVersion 2>/dev/null || true)
    [ -n "$have" ] || fail "could not read the macOS version with sw_vers"
    at_least "$have" "$MACOS_MIN" ||
        fail "Allot needs macOS $MACOS_MIN or newer, and this is $have"
}

# Prints the release target of this machine, or fails with why there is none.
detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case $os in
    Linux)
        case $arch in
        x86_64 | amd64) arch=x86_64 ;;
        aarch64 | arm64) arch=aarch64 ;;
        *) fail "Linux on $arch is not supported: packages exist for x86_64 and aarch64" ;;
        esac
        check_glibc
        printf '%s-unknown-linux-gnu\n' "$arch"
        ;;
    Darwin)
        # A shell under Rosetta reports x86_64 on an Apple Silicon Mac.
        if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = 1 ]; then
            arch=arm64
        fi
        [ "$arch" = arm64 ] ||
            fail "macOS on $arch is not supported: the package is built for Apple Silicon"
        check_macos
        printf 'aarch64-apple-darwin\n'
        ;;
    MINGW* | MSYS* | CYGWIN*)
        fail "on Windows, run this in PowerShell instead:" \
            "irm https://raw.githubusercontent.com/meh7an/allot/main/install.ps1 | iex"
        ;;
    *) fail "$os is not supported: Allot runs on Linux, macOS and Windows" ;;
    esac
}

check_version() {
    printf '%s\n' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' ||
        fail "'$1' is not a release version such as $FIRST_VERSION"
    at_least "$1" "$FIRST_VERSION" ||
        fail "Allot $1 has no Linux or macOS package; the first is $FIRST_VERSION"
}

# Prints the latest release's version, read from where releases/latest redirects to.
latest_version() {
    url=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$1/latest") ||
        fail "could not reach $1/latest"
    tag=${url##*/}
    case $tag in
    v[0-9]*) printf '%s\n' "${tag#v}" ;;
    *) fail "could not tell the latest release from $url" ;;
    esac
}

sha256_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{ print $1 }'
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | awk '{ print $1 }'
    else
        fail "this script needs sha256sum or shasum to check the download"
    fi
}

# Fails unless the file $1/$2 matches the checksum in $1/$2.sha256.
verify() {
    listed=$(awk '{ print $1; exit }' "$1/$2.sha256")
    name=$(awk '{ sub(/^\*/, "", $2); print $2; exit }' "$1/$2.sha256")
    [ "$name" = "$2" ] || fail "$2.sha256 does not hold a checksum for $2"
    actual=$(sha256_of "$1/$2")
    [ "$actual" = "$listed" ] ||
        fail "checksum mismatch for $2: expected $listed, got $actual. Nothing was installed."
    say "checked $2 against its .sha256"
}

# Says what to do when the link `allot install` made is not on this shell's PATH.
path_hint() {
    [ -e "$HOME/.local/bin/allot" ] || return 0
    case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) say "~/.local/bin is not on PATH here; add it in your shell's profile to type allot" ;;
    esac
}

main() {
    version=${ALLOT_VERSION:-}
    count=$#
    while [ "$count" -gt 0 ]; do
        arg=$1
        shift
        count=$((count - 1))
        case $arg in
        --version)
            [ "$count" -gt 0 ] || fail "--version needs a value, such as $FIRST_VERSION"
            version=$1
            shift
            count=$((count - 1))
            ;;
        --version=*) version=${arg#*=} ;;
        -h | --help)
            usage
            return 0
            ;;
        *) set -- "$@" "$arg" ;;
        esac
    done

    need uname
    need curl
    need tar
    need awk
    target=$(detect_target)

    base=${ALLOT_DOWNLOAD_BASE:-https://github.com/meh7an/allot/releases}
    base=${base%/}
    if [ -z "$version" ]; then
        version=$(latest_version "$base")
    fi
    version=${version#v}
    check_version "$version"

    package=allot-$version-$target
    asset=$package.tar.gz
    work=$(mktemp -d 2>/dev/null || mktemp -d -t allot-install)
    trap 'rm -rf "$work"' EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    say "downloading Allot $version for $target"
    for file in "$asset" "$asset.sha256"; do
        curl -fsSL --retry 2 -o "$work/$file" "$base/download/v$version/$file" ||
            fail "could not download $base/download/v$version/$file"
    done
    verify "$work" "$asset"

    tar -xzf "$work/$asset" -C "$work"
    [ -x "$work/$package/allot" ] || fail "$asset has no $package/allot"

    "$work/$package/allot" install "$@"

    path_hint
    if [ "$target" = aarch64-apple-darwin ]; then
        say "the menu bar app is a separate download:" \
            "https://github.com/meh7an/allot/blob/main/docs/install.md#the-menu-bar-app"
    fi
    say "Allot $version is installed. Start a new Claude Code session to use it."
}

main "$@"
