#!/usr/bin/env bash
# Packs the npm packages from a published release: downloads each archive npm/pack.mjs needs,
# checks it against its .sha256 and the digest GitHub recorded, unpacks it, and has pack.mjs
# write the tarballs into <out dir>. Needs gh, node, npm, unzip and GITHUB_REPOSITORY.
#   npm-packages.sh <tag> <out dir>
set -euo pipefail

tag=$1
out=$2
version=${tag#v}
here=$(cd "$(dirname "$0")" && pwd)
pack="$here/../../npm/pack.mjs"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

sha256() {
    if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

assets=$(node "$pack" assets "$version")
while read -r asset <&3; do
    hash=$(bash "$here/asset-sha256.sh" "$tag" "$asset")
    gh release download "$tag" --repo "$GITHUB_REPOSITORY" --pattern "$asset" --dir "$work"
    echo "$hash  $work/$asset" | sha256 --check --strict
    case $asset in
        *.zip) unzip -q "$work/$asset" -d "$work" ;;
        *) tar -xzf "$work/$asset" -C "$work" ;;
    esac
done </dev/null 3<<<"$assets"

node "$pack" pack "$version" "$work" "$out"
