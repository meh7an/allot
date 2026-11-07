#!/usr/bin/env bash
# Rewrites the tap's formula and cask for one version. A tap already on a newer version is left
# alone, so re-running an old tag never takes Homebrew users backward.
#   update-tap.sh <tap dir> <version> <tarball sha256> <app zip sha256>
set -euo pipefail

tap=$1
version=$2
tarball_sha=$3
app_sha=$4

formula=$tap/Formula/allot.rb
cask=$tap/Casks/allot-app.rb
downloads=https://github.com/meh7an/allot/releases/download

# A layout this script does not expect has to stop the job, not leave a half-updated tap.
expect_one() {
  local file=$1 pattern=$2
  if [ "$(grep -cE "$pattern" "$file")" != 1 ]; then
    echo "::error::$file does not have exactly one line matching $pattern" >&2
    exit 1
  fi
}

url_line="^  url \"$downloads/v[0-9.]+/allot-[0-9.]+-aarch64-apple-darwin\.tar\.gz\"$"
sha_line='^  sha256 "[0-9a-f]{64}"$'
version_line='^  version "[0-9.]+"$'
expect_one "$formula" "$url_line"
expect_one "$formula" "$sha_line"
expect_one "$cask" "$version_line"
expect_one "$cask" "$sha_line"

current=$(sed -nE 's/^  version "([0-9.]+)"$/\1/p' "$cask")
newest=$(printf '%s\n%s\n' "$current" "$version" | sort -V | tail -n 1)
if [ "$newest" != "$version" ]; then
  echo "::notice::The tap is on $current, newer than $version, so it stays there"
  exit 0
fi

url="$downloads/v$version/allot-$version-aarch64-apple-darwin.tar.gz"
sed -i -E \
  -e "s#$url_line#  url \"$url\"#" \
  -e "s#$sha_line#  sha256 \"$tarball_sha\"#" \
  "$formula"
sed -i -E \
  -e "s#$version_line#  version \"$version\"#" \
  -e "s#$sha_line#  sha256 \"$app_sha\"#" \
  "$cask"
