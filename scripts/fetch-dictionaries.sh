#!/usr/bin/env bash
# Download the Hunspell dictionaries shipped with Cardfile into packaging/dictionaries.
# Source: https://github.com/wooorm/dictionaries (dictionary-en, SCOWL-based; see its license file).
set -euo pipefail
DEST="$(cd "$(dirname "$0")/.." && pwd)/packaging/dictionaries"
REV="${DICTIONARIES_REV:-main}"
BASE="https://raw.githubusercontent.com/wooorm/dictionaries/${REV}/dictionaries"
mkdir -p "$DEST"
fetch() { # <lang dir> <file name prefix>
  curl -fsSL "$BASE/$1/index.aff" -o "$DEST/$2.aff"
  curl -fsSL "$BASE/$1/index.dic" -o "$DEST/$2.dic"
  curl -fsSL "$BASE/$1/license" -o "$DEST/LICENSE-$2"
}
fetch en en_US
echo "dictionaries in $DEST:"; ls -l "$DEST"
