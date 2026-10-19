#!/usr/bin/env bash
# Download the Hunspell dictionaries shipped with Cardfile into packaging/dictionaries.
# Source: https://github.com/wooorm/dictionaries: dictionary-en (en_US, SCOWL-based) and
# dictionary-es (es_ES, used under MPL-1.1 of its GPL-3.0/LGPL-3.0/MPL-1.1 choice); see their
# license files. The other languages are downloaded by the app on request (crates/cardfile-ui/src/dictionaries.rs).
#
# The files are pinned to one commit of that repository and checked against their SHA-256,
# so a change upstream can never slip into a release. To update: pick a new commit, run
# with DICTIONARIES_REV=<sha> DICTIONARIES_UPDATE=1 to print the new hashes, then update
# REV and the hashes below.
set -euo pipefail
DEST="$(cd "$(dirname "$0")/.." && pwd)/packaging/dictionaries"
REV="${DICTIONARIES_REV:-8cfea406b505e4d7df52d5a19bce525df98c54ab}"
BASE="https://raw.githubusercontent.com/wooorm/dictionaries/${REV}/dictionaries"
declare -A SHA256=(
  [en/index.aff]=8ae1f19d4840d957728ad90555d5a8dff6cc5c046279c95ff0c00fc0a0136c7b
  [en/index.dic]=f0b1a234bd178bdd01875b2a392a9647f888b8fe879f79c52aae62c2759b3647
  [en/license]=2a7e8d8ae9e8facc84818546ae2a8d83aec5e9c80a675ff789acd1c338b53b3d
  [es/index.aff]=892039d59db11d747061b81c3a92a52d48ba0e055b5835af1943cbd3c2fba85b
  [es/index.dic]=907f786a8ceb3456722b20ad91dd4dbe99c5c16e45015ea63155e36f26b06d2c
  [es/license]=a21581497f07b5e550e438d26890ea2b1704492357de67740613bd589b1c652a
)
mkdir -p "$DEST"
get() { # <path in the repository> <destination>
  local tmp
  tmp="$(mktemp "$DEST/.download.XXXXXX")"
  curl -fsSL --retry 3 "$BASE/$1" -o "$tmp"
  local actual
  actual="$(sha256sum "$tmp" | cut -d' ' -f1)"
  if [ "${DICTIONARIES_UPDATE:-}" = 1 ]; then
    echo "  [$1]=$actual"
  elif [ "$actual" != "${SHA256[$1]:-}" ]; then
    rm -f "$tmp"
    echo "checksum mismatch for $1 at $REV: got $actual" >&2
    exit 1
  fi
  mv -f "$tmp" "$2"
}
fetch() { # <lang dir> <file name prefix>
  get "$1/index.aff" "$DEST/$2.aff"
  get "$1/index.dic" "$DEST/$2.dic"
  get "$1/license" "$DEST/LICENSE-$2"
}
fetch en en_US
fetch es es_ES
echo "dictionaries in $DEST:"; ls -l "$DEST"
