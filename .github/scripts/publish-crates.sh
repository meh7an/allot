#!/usr/bin/env bash
# Publishes every publishable workspace crate at the checked-out version, skipping any that
# crates.io already has, so a failed run can be run again. One `cargo publish` takes them all,
# since a crate cannot be packaged alone while a sibling it needs is not on crates.io yet.
#   publish-crates.sh [--dry-run]
# A dry run packages every crate. When crates.io already has the version, cargo would build
# against those published crates instead of these sources, so the dry run packages under a
# throwaway pre-release of it (`<version>-ci`) and puts the manifests and lockfile back after.
set -euo pipefail

dry_run=
case ${1:-} in
    '') ;;
    --dry-run) dry_run=1 ;;
    *)
        echo "usage: publish-crates.sh [--dry-run]" >&2
        exit 2
        ;;
esac

work=$(mktemp -d)
restore() {
    if [ -d "$work/backup" ]; then
        cp -R "$work/backup/." .
    fi
    rm -rf "$work"
}
trap restore EXIT

# True when crates.io's index lists this version; a 404 means the crate does not exist yet.
published() {
    local name=$1 version=$2 status
    status=$(curl --silent --show-error --location --output "$work/index" \
        --write-out '%{http_code}' \
        "https://index.crates.io/${name:0:2}/${name:2:2}/$name")
    case $status in
        200) jq -r .vers <"$work/index" | tr -d '\r' | grep -xF "$version" >/dev/null ;;
        404) return 1 ;;
        *)
            echo "::error::crates.io's index answered $status for $name" >&2
            exit 1
            ;;
    esac
}

# Rewrites the workspace version from $1 to $2 and pins every path dependency to exactly $2,
# keeping copies to restore, then brings the lockfile's workspace entries in line. The pin
# matters: `^0.3.0-ci` also matches the published 0.3.0, which cargo would pick instead.
stand_in() {
    local from=${1//./\\.} to=$2 file
    while read -r file; do
        mkdir -p "$work/backup/$(dirname "$file")"
        cp "$file" "$work/backup/$file"
        sed -e "s/^version = \"$from\"$/version = \"$to\"/" \
            -e "s/version = \"$from\", path/version = \"=$to\", path/" \
            "$file" >"$work/manifest"
        cp "$work/manifest" "$file"
    done < <(git ls-files -- Cargo.toml '*/Cargo.toml')
    cp Cargo.lock "$work/backup/Cargo.lock"
    cargo update --workspace --quiet
}

# Crates with `publish = false` report an empty list; the others report null.
packages=$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.publish != []) | "\(.name) \(.version)"' | tr -d '\r')

arguments=()
release=
any_published=
while read -r name version; do
    [ -n "$name" ] || continue
    release=$version
    if published "$name" "$version"; then
        any_published=1
        if [ -z "$dry_run" ]; then
            echo "::notice::crates.io already has $name $version"
            continue
        fi
    fi
    arguments+=(--package "$name")
done <<<"$packages"

if [ "${#arguments[@]}" -eq 0 ]; then
    echo "::notice::No crate to publish"
    exit 0
fi

if [ -n "$dry_run" ]; then
    arguments+=(--dry-run)
    # cargo verifies each crate against its packaged siblings, which it unpacks under
    # CARGO_HOME and reuses when one of that name and version is already there. The CI cache
    # keeps the registry, so a copy an earlier run unpacked at the same version would be built
    # instead of these sources; drop those first.
    cargo_home=${CARGO_HOME:-$HOME/.cargo}
    while read -r name version; do
        [ -n "$name" ] || continue
        for stale in "$version" "$version-ci"; do
            rm -rf "$cargo_home"/registry/src/*/"$name-$stale" \
                "$cargo_home"/registry/cache/*/"$name-$stale.crate"
        done
    done <<<"$packages"
    if [ -n "$any_published" ]; then
        echo "::notice::crates.io already has $release; packaging these sources as $release-ci"
        stand_in "$release" "$release-ci"
        # The stand-in version is the only uncommitted change.
        arguments+=(--allow-dirty)
    fi
fi

cargo publish --locked "${arguments[@]}"
