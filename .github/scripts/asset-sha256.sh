#!/usr/bin/env bash
# Prints a release asset's SHA-256 from its .sha256 file, once that matches the digest GitHub
# recorded for the asset. So a channel never points at a missing or mislabelled file.
#   asset-sha256.sh <tag> <asset name>
set -euo pipefail

tag=$1
asset=$2

line=$(gh release download "$tag" --repo "$GITHUB_REPOSITORY" --pattern "$asset.sha256" --output -)
read -r hash name <<<"$line"
if [[ ! "$hash" =~ ^[0-9a-f]{64}$ || "$name" != "$asset" ]]; then
  echo "::error::$asset.sha256 does not hold a checksum for $asset" >&2
  exit 1
fi

digest=$(gh api "repos/$GITHUB_REPOSITORY/releases/tags/$tag" \
  --jq ".assets[] | select(.name == \"$asset\") | .digest")
if [ "$digest" != "sha256:$hash" ]; then
  echo "::error::$asset has digest '$digest', but its .sha256 file says $hash" >&2
  exit 1
fi

printf '%s\n' "$hash"
