#!/usr/bin/env bash
# Prints the checklist issue for the stores that cannot be updated by CI. When a store gets
# listed, drop the last argument of its `store` line and its entry becomes real steps.
#   store-checklist.sh <tag> <version>
set -euo pipefail

tag=$1
version=$2
here=$(dirname "$0")
downloads=https://github.com/$GITHUB_REPOSITORY/releases/download/$tag

# store <name> <asset> <upload page> [the milestone that will list it]
store() {
  local name=$1 asset=$2 page=$3 listing=${4:-} sha
  sha=$(bash "$here/asset-sha256.sh" "$tag" "$asset")
  printf '## %s\n\n' "$name"
  if [ -n "$listing" ]; then
    printf 'Not listed yet (%s), so nothing to do for this release.\n\n' "$listing"
  else
    printf -- '- [ ] Upload the file below\n- [ ] It passed review and is live\n\n'
  fi
  printf -- '- File: [%s](%s/%s)\n' "$asset" "$downloads" "$asset"
  printf -- '- SHA-256: `%s`\n' "$sha"
  printf -- '- Upload page: %s\n\n' "$page"
}

printf 'What %s still needs by hand. Close this once it is done.\n\n' "$tag"
store "GNOME Extensions" "allot-$version-gnome-extension.zip" \
  https://extensions.gnome.org/upload/ M3.1
store "KDE Store" "allot-$version-kde-widget.plasmoid" https://store.kde.org/ M3.2
