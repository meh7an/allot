#!/usr/bin/env bash
# Checks the cargo-binstall metadata of the three binary crates against the real release of the
# workspace's version: for every target binstall must fetch the archive by the metadata alone and
# find the binary in it. Linux x86_64 only, since binstall can fetch for any target from there.
# A version with no release yet, as on the commit that bumps it, is skipped.
#   test-binstall.sh
set -euo pipefail

binstall_version=1.26.2
binstall_sha256=9e0e064f189a9abb9f8df5fb93c3c810ef5cc19e7212d998c7a64c9e4d3527d5
targets='x86_64-pc-windows-msvc x86_64-unknown-linux-gnu
aarch64-unknown-linux-gnu aarch64-apple-darwin'

version=$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.name == "allot-cli") | .version')
if ! gh release view "v$version" --repo meh7an/allot >/dev/null 2>&1; then
    echo "::notice::v$version has no release yet, so there is nothing to check binstall against"
    exit 0
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
archive=cargo-binstall-x86_64-unknown-linux-musl.tgz
curl --fail --silent --show-error --location --output "$work/$archive" \
    "https://github.com/cargo-bins/cargo-binstall/releases/download/v$binstall_version/$archive"
echo "$binstall_sha256  $work/$archive" | sha256sum --check --strict
tar -xzf "$work/$archive" -C "$work"

for target in $targets; do
    case $target in
        *windows*) exe=.exe ;;
        *) exe='' ;;
    esac
    root="$work/$target"
    while read -r crate directory binary; do
        "$work/cargo-binstall" --manifest-path "$directory" --targets "$target" --no-confirm \
            --disable-strategies compile,quick-install --no-track --root "$root" "$crate"
        test -f "$root/bin/$binary$exe"
        echo "ok: $crate $version for $target has $binary$exe"
    done <<'CRATES'
allot-cli cli allot
allotd daemon allotd
allot-mcp mcp allot-mcp
CRATES
done
