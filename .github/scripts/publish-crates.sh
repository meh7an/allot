#!/usr/bin/env bash
# Publishes every publishable workspace crate at the checked-out version, skipping any that
# crates.io already has, so a failed run can be run again. One `cargo publish` takes them all,
# since a crate cannot be packaged alone while a sibling it needs is not on crates.io yet.
#   publish-crates.sh [--dry-run]
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
trap 'rm -rf "$work"' EXIT

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

# Crates with `publish = false` report an empty list; the others report null.
packages=$(cargo metadata --no-deps --format-version 1 \
    | jq -r '.packages[] | select(.publish != []) | "\(.name) \(.version)"' | tr -d '\r')

arguments=()
while read -r name version; do
    [ -n "$name" ] || continue
    if published "$name" "$version"; then
        echo "::notice::crates.io already has $name $version"
    else
        arguments+=(--package "$name")
    fi
done <<<"$packages"

if [ "${#arguments[@]}" -eq 0 ]; then
    echo "::notice::No crate to publish"
    exit 0
fi

cargo publish --locked ${dry_run:+--dry-run} "${arguments[@]}"
