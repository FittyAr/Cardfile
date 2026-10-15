#!/bin/sh
# Cardfile installer for Linux (x86_64 and aarch64).
#
#   curl -fsSL https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.sh | sh
#   ... | sh -s -- [--version v2.0.0] [--stable] [--no-desktop-shortcut] [--uninstall] [--verify-only] [--insecure-skip-signature] [--yes]
#
# Always installs the newest published release (not a fixed version): the
# newest stable one, or the newest pre-release while no stable release is
# newer. --stable ignores pre-releases; --version installs that exact tag.
#
# Installs the program to ~/.local/lib/FittyAr/Cardfile (all FittyAr apps share the
# FittyAr folder) with a `cardfile` command in
# ~/.local/bin, an application-menu entry (.desktop file + icons) and a
# desktop shortcut. No root needed. The cards live in
# ${XDG_DATA_HOME:-~/.local/share}/FittyAr/Cardfile. Cardfile updates itself afterwards
# (Settings -> General).
#
# The download is checked against the signed SHA256SUMS and its own minisign
# signature. When minisign is not installed, its official release
# (jedisct1/minisign) is downloaded into the temporary folder, pinned by
# version and SHA-256 below.
#
# Everything runs from `main` on the last line, so a download cut short
# executes nothing.
set -eu

REPO="FittyAr/Cardfile"
APP_ID="ar.fittyar.Cardfile"
# Cardfile release signing key (minisign). Must match PUBLIC_KEY in
# crates/cardfile-updater/src/lib.rs and install.ps1.
MINISIGN_PUBKEY="RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w"
# https://github.com/jedisct1/minisign/releases (same version as install.ps1)
MINISIGN_VERSION="0.12"
MINISIGN_TARBALL_SHA256="9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73"

DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
VENDOR_DIR="$HOME/.local/lib/FittyAr"
LIB_DIR="$VENDOR_DIR/Cardfile"
# Used by this script before installs moved under the FittyAr folder.
PREVIOUS_DIR="$HOME/.local/lib/cardfile"
BIN_DIR="$HOME/.local/bin"
APPS_DIR="$DATA_HOME/applications"
ICONS_DIR="$DATA_HOME/icons/hicolor"
DESKTOP_FILE="$APPS_DIR/$APP_ID.desktop"
# Where Cardfile keeps the cards (asked before deleting on uninstall), in the
# shared FittyAr folder. Cardfile moves the folder of the earlier layout there
# on its next start.
CARDS_VENDOR_DIR="$DATA_HOME/FittyAr"
CARDS_DIR="$CARDS_VENDOR_DIR/Cardfile"
PREVIOUS_CARDS_DIR="$DATA_HOME/cardfile"
# Installed by the cargo-dist installer of 2.0.0-alpha.1 to 2.0.0-alpha.3.
LEGACY_DIR="$HOME/.cardfile/bin"
ICON_SIZES="16 24 32 48 64 128 256 512"

VERSION=""
STABLE=false
DESKTOP_SHORTCUT=true
UNINSTALL_MODE=false
VERIFY_ONLY=false
SKIP_SIGNATURE=false
ASSUME_YES=false
TEMP_DIR=""

# Colours only on a terminal.
if [ -t 1 ]; then
    ESC=$(printf '\033')
    RED="${ESC}[0;31m"
    GREEN="${ESC}[0;32m"
    BLUE="${ESC}[0;34m"
    YELLOW="${ESC}[1;33m"
    NC="${ESC}[0m"
else
    RED="" GREEN="" BLUE="" YELLOW="" NC=""
fi

die() {
    echo "${RED}Error: $*${NC}" >&2
    exit 1
}

cleanup() {
    if [ -n "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}

usage() {
    echo "Usage: install.sh [--version vX.Y.Z] [--stable] [--no-desktop-shortcut] [--uninstall] [--verify-only] [--insecure-skip-signature] [--yes]"
}

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --version)
                [ $# -ge 2 ] || die "--version needs a tag, e.g. --version v2.0.0"
                VERSION="$2"
                shift
                ;;
            --stable | stable) STABLE=true ;;
            --no-desktop-shortcut) DESKTOP_SHORTCUT=false ;;
            --uninstall | uninstall) UNINSTALL_MODE=true ;;
            --verify-only) VERIFY_ONLY=true ;;
            --insecure-skip-signature) SKIP_SIGNATURE=true ;;
            -y | --yes) ASSUME_YES=true ;;
            -h | --help)
                usage
                exit 0
                ;;
            *)
                usage >&2
                die "Unknown argument '$1'."
                ;;
        esac
        shift
    done
}

# ask "question" -> 0 for yes. Without a terminal (CI, pipes) the answer is no,
# unless --yes was given.
ask() {
    if [ "$ASSUME_YES" = "true" ]; then
        return 0
    fi
    if ! { [ -c /dev/tty ] && (: < /dev/tty) 2> /dev/null; }; then
        return 1
    fi
    printf "%s [y/N]: " "$1"
    read -r ANSWER < /dev/tty || ANSWER="n"
    case "$ANSWER" in
        [yY] | [yY][eE][sS]) return 0 ;;
        *) return 1 ;;
    esac
}

refresh_menus() {
    if command -v update-desktop-database > /dev/null 2>&1; then
        update-desktop-database -q "$APPS_DIR" 2> /dev/null || true
    fi
    if command -v gtk-update-icon-cache > /dev/null 2>&1; then
        gtk-update-icon-cache -q -t -f "$ICONS_DIR" 2> /dev/null || true
    fi
}

# The user's desktop folder (localized names included), if there is one.
desktop_dir() {
    DIR=""
    if command -v xdg-user-dir > /dev/null 2>&1; then
        DIR=$(xdg-user-dir DESKTOP 2> /dev/null || true)
    fi
    if [ -z "$DIR" ] || [ "$DIR" = "$HOME" ]; then
        DIR="$HOME/Desktop"
    fi
    printf '%s' "$DIR"
}

# ─── Uninstall ───────────────────────────────────────────────────────────────

uninstall() {
    echo "${BLUE}Cardfile uninstaller${NC}"
    echo "=============================="
    if [ ! -d "$LIB_DIR" ] && [ ! -f "$DESKTOP_FILE" ]; then
        echo "Cardfile is not installed in $LIB_DIR."
    elif ask "Uninstall Cardfile from $LIB_DIR?"; then
        rm -rf "$LIB_DIR"
        # Leave the shared FittyAr folder only while other apps use it.
        rmdir "$VENDOR_DIR" 2> /dev/null || true
        # Only our own link: never a cardfile installed some other way.
        if [ -L "$BIN_DIR/cardfile" ]; then
            rm -f "$BIN_DIR/cardfile"
        fi
        rm -f "$DESKTOP_FILE" "$(desktop_dir)/$APP_ID.desktop"
        for size in $ICON_SIZES; do
            rm -f "$ICONS_DIR/${size}x${size}/apps/$APP_ID.png"
        done
        refresh_menus
        echo "${GREEN}Cardfile was removed.${NC}"
    else
        echo "Uninstall cancelled."
        return 0
    fi
    for cards in "$CARDS_DIR" "$PREVIOUS_CARDS_DIR"; do
        [ -d "$cards" ] && [ "$ASSUME_YES" != "true" ] || continue
        if ask "Also delete your cards and settings in $cards? This cannot be undone."; then
            rm -rf "$cards"
            echo "Your cards and settings were deleted."
        else
            echo "Your cards and settings were kept in $cards."
        fi
    done
    # Leave the shared FittyAr folder only while other apps use it.
    rmdir "$CARDS_VENDOR_DIR" 2> /dev/null || true
}

# ─── Install ─────────────────────────────────────────────────────────────────

detect_platform() {
    [ "$(uname -s)" = "Linux" ] || die "This script only supports Linux."
    case "$(uname -m)" in
        x86_64 | amd64)
            TARGET="x86_64-unknown-linux-gnu"
            MINISIGN_ARCH="x86_64"
            ;;
        aarch64 | arm64)
            TARGET="aarch64-unknown-linux-gnu"
            MINISIGN_ARCH="aarch64"
            ;;
        *) die "Unsupported architecture '$(uname -m)'. Releases exist for x86_64 and aarch64." ;;
    esac
}

check_dependencies() {
    for cmd in curl tar xz; do
        command -v "$cmd" > /dev/null 2>&1 || die "Required command '$cmd' is not installed."
    done
    if ! command -v sha256sum > /dev/null 2>&1 && ! command -v shasum > /dev/null 2>&1; then
        die "'sha256sum' or 'shasum' is required to verify the download."
    fi
}

sha256_of() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

find_minisign() {
    if command -v minisign > /dev/null 2>&1; then
        MINISIGN=minisign
        return 0
    fi
    echo "minisign is not installed; downloading minisign ${MINISIGN_VERSION}..."
    MS_TARBALL="${TEMP_DIR}/minisign-${MINISIGN_VERSION}-linux.tar.gz"
    curl -fsSL "https://github.com/jedisct1/minisign/releases/download/${MINISIGN_VERSION}/minisign-${MINISIGN_VERSION}-linux.tar.gz" \
        -o "$MS_TARBALL" || die "Could not download minisign ${MINISIGN_VERSION}."
    [ "$(sha256_of "$MS_TARBALL")" = "$MINISIGN_TARBALL_SHA256" ] \
        || die "The minisign download does not match its pinned SHA-256. Aborting."
    mkdir -p "${TEMP_DIR}/minisign"
    tar -xzf "$MS_TARBALL" -C "${TEMP_DIR}/minisign" 2> /dev/null \
        || die "Could not extract minisign ${MINISIGN_VERSION}."
    MINISIGN="${TEMP_DIR}/minisign/minisign-linux/${MINISIGN_ARCH}/minisign"
    [ -f "$MINISIGN" ] || die "minisign not found in the minisign ${MINISIGN_VERSION} archive."
    chmod +x "$MINISIGN"
}

# Sets VERSION (the tag) and RELEASE_JSON. GitHub lists releases newest first;
# without --stable the first non-draft one wins, pre-releases included.
pick_release() {
    echo "Looking for the newest Cardfile release..."
    if [ -n "$VERSION" ]; then
        case "$VERSION" in v*) ;; *) VERSION="v$VERSION" ;; esac
        RELEASE_JSON=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/tags/${VERSION}") \
            || die "Release ${VERSION} not found."
    else
        ALL_JSON=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases?per_page=30") \
            || die "Could not reach the GitHub API."
        # One release object per line is not guaranteed, so walk the fields in
        # order: every release has tag_name, then draft and prerelease.
        VERSION=$(printf '%s\n' "$ALL_JSON" | awk -v stable="$STABLE" '
            /"tag_name":/ { gsub(/.*"tag_name": *"|".*/, ""); tag = $0 }
            /"draft":/ { draft = ($0 ~ /true/) }
            /"prerelease":/ {
                # A tag with a suffix (v2.0.0-alpha.3) is a pre-release too.
                pre = ($0 ~ /true/) || (tag ~ /-/)
                # 0.x and 1.x were the old Python app, without these archives.
                if (tag != "" && tag !~ /^v[01]\./ && !draft && (stable != "true" || !pre)) { print tag; exit }
                tag = ""
            }')
        if [ -z "$VERSION" ]; then
            if [ "$STABLE" = "true" ]; then
                die "There is no stable release yet. Run without --stable to install the newest pre-release."
            fi
            die "No published release found."
        fi
        RELEASE_JSON=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/tags/${VERSION}") \
            || die "Could not read release ${VERSION}."
    fi
    case "$VERSION" in
        v[0-9]*.[0-9]*.[0-9]*) ;;
        *) die "Unexpected release tag '${VERSION}'." ;;
    esac
    case "$VERSION" in
        *[!A-Za-z0-9.-]*) die "Unexpected release tag '${VERSION}'." ;;
    esac
    if printf '%s\n' "$RELEASE_JSON" | grep -q '"prerelease": *true' || [ "${VERSION#*-}" != "$VERSION" ]; then
        echo "Newest version: ${GREEN}${VERSION}${NC} (pre-release)"
    else
        echo "Newest version: ${GREEN}${VERSION}${NC}"
    fi
}

# verify <file> <published name>: SHA-256 from the signed SHA256SUMS, then the
# minisign signatures of both files (trusted comment names the tag and file).
verify() {
    echo "Verifying SHA-256 checksum..."
    curl -fsSL "${BASE}/SHA256SUMS" -o "${TEMP_DIR}/SHA256SUMS" || die "Could not download SHA256SUMS."
    EXPECTED=$(awk -v n="$2" '{ f = $NF; sub(/^\*/, "", f); if (f == n) { print tolower($1); exit } }' "${TEMP_DIR}/SHA256SUMS")
    [ -n "$EXPECTED" ] || die "$2 is not listed in SHA256SUMS."
    [ "$EXPECTED" = "$(sha256_of "$1")" ] || die "SHA-256 checksum mismatch for $2. Aborting."

    if [ "$SKIP_SIGNATURE" = "true" ]; then
        echo "${YELLOW}Warning: --insecure-skip-signature given; the signatures were not checked.${NC}"
        return 0
    fi
    find_minisign
    echo "Verifying minisign signatures..."
    for pair in "${TEMP_DIR}/SHA256SUMS|SHA256SUMS" "$1|$2"; do
        FILE=${pair%%|*}
        NAME=${pair#*|}
        if ! curl -fsSL "${BASE}/${NAME}.minisig" -o "${FILE}.minisig" \
            || ! SIG_OUT=$("$MINISIGN" -V -P "$MINISIGN_PUBKEY" -m "$FILE" -x "${FILE}.minisig") \
            || ! printf '%s\n' "$SIG_OUT" | tr -s ' \t' '  ' | grep -qF "Cardfile ${VERSION} release file: ${NAME}"; then
            die "minisign signature verification failed for ${NAME}. Aborting."
        fi
    done
}

# Sets PKG_DIR (the extracted folder holding `cardfile`).
download_release() {
    TARBALL="cardfile-${TARGET}.tar.xz"
    printf '%s\n' "$RELEASE_JSON" | grep -qE "\"name\": *\"${TARBALL}\"" \
        || die "Release ${VERSION} has no ${TARBALL}."
    BASE="https://github.com/${REPO}/releases/download/${VERSION}"
    echo "Downloading ${TARBALL}..."
    curl -fsSL "${BASE}/${TARBALL}" -o "${TEMP_DIR}/${TARBALL}" || die "Download failed."
    verify "${TEMP_DIR}/${TARBALL}" "$TARBALL"

    echo "Extracting..."
    mkdir "${TEMP_DIR}/extract"
    tar -xJf "${TEMP_DIR}/${TARBALL}" -C "${TEMP_DIR}/extract"
    BIN_SRC=$(find "${TEMP_DIR}/extract" -type f -name cardfile | head -n 1)
    [ -n "$BIN_SRC" ] || die "cardfile not found in ${TARBALL}."
    PKG_DIR=$(dirname "$BIN_SRC")
}

# Icons and the menu entry come from the repository at the installed tag.
install_desktop_integration() {
    RAW="https://raw.githubusercontent.com/${REPO}/${VERSION}/packaging/linux"
    for size in $ICON_SIZES; do
        DEST="$ICONS_DIR/${size}x${size}/apps"
        mkdir -p "$DEST"
        curl -fsSL "$RAW/icons/${size}x${size}/${APP_ID}.png" -o "$DEST/$APP_ID.png" 2> /dev/null \
            || rm -f "$DEST/$APP_ID.png"
    done
    mkdir -p "$APPS_DIR"
    cat > "$DESKTOP_FILE" << EOF
[Desktop Entry]
Type=Application
Name=Cardfile
GenericName=Card file
Comment=A modern digital card file for your notes
Exec=$LIB_DIR/cardfile
TryExec=$LIB_DIR/cardfile
Icon=$APP_ID
Terminal=false
Categories=Office;
Keywords=notes;cards;notebook;editor;
StartupWMClass=$APP_ID
EOF
    chmod 644 "$DESKTOP_FILE"
    refresh_menus

    if [ "$DESKTOP_SHORTCUT" = "true" ]; then
        DESK=$(desktop_dir)
        if [ -d "$DESK" ]; then
            cp "$DESKTOP_FILE" "$DESK/$APP_ID.desktop"
            chmod 755 "$DESK/$APP_ID.desktop"
            # GNOME only launches desktop files marked as trusted.
            if command -v gio > /dev/null 2>&1; then
                gio set "$DESK/$APP_ID.desktop" metadata::trusted true 2> /dev/null || true
            fi
            DESKTOP_DONE=true
        fi
    fi
}

install_files() {
    echo "Installing to $LIB_DIR..."
    mkdir -p "$LIB_DIR" "$BIN_DIR"
    # Copy next to the target and rename over it: a running Cardfile keeps its
    # old file (no "Text file busy"), and a failed copy changes nothing.
    STAGE="$LIB_DIR/.cardfile.new.$$"
    cp "$BIN_SRC" "$STAGE"
    chmod 755 "$STAGE"
    for item in "$PKG_DIR"/* ; do
        [ "$item" = "$BIN_SRC" ] && continue
        cp -R "$item" "$LIB_DIR/"
    done
    mv -f "$STAGE" "$LIB_DIR/cardfile"
    ln -sf "$LIB_DIR/cardfile" "$BIN_DIR/cardfile"
}

remove_legacy() {
    # The earlier layout of this same installer: replaced silently.
    if [ -f "$PREVIOUS_DIR/cardfile" ]; then
        echo "Moving from $PREVIOUS_DIR to $LIB_DIR..."
        rm -rf "$PREVIOUS_DIR"
    fi
    [ -f "$LEGACY_DIR/cardfile" ] || return 0
    echo "An older Cardfile installation was found in $LEGACY_DIR."
    if ask "Remove it (your cards are kept)?"; then
        rm -rf "$LEGACY_DIR"
        rmdir "$(dirname "$LEGACY_DIR")" 2> /dev/null || true
        echo "Removed. If your shell profile adds $LEGACY_DIR to PATH, you can delete that line."
    fi
}

check_runtime() {
    # GPUI needs Vulkan and xkbcommon at run time; warn early instead of failing silently.
    if command -v ldd > /dev/null 2>&1; then
        MISSING=$(ldd "$LIB_DIR/cardfile" 2> /dev/null | awk '/not found/ {print $1}' | tr '\n' ' ')
        if [ -n "$MISSING" ]; then
            echo "${YELLOW}Missing system libraries: ${MISSING}${NC}"
            echo "On Debian/Ubuntu: sudo apt install libxkbcommon0 libxkbcommon-x11-0 libvulkan1 libasound2"
        fi
    fi
}

print_summary() {
    echo "=============================="
    echo "${GREEN}Cardfile ${VERSION} installed.${NC}"
    echo "  Program:   ${BLUE}${LIB_DIR}/cardfile${NC}"
    echo "  Command:   ${BLUE}${BIN_DIR}/cardfile${NC}"
    if [ "${DESKTOP_DONE:-false}" = "true" ]; then
        echo "  Shortcuts: application menu and desktop"
    else
        echo "  Shortcuts: application menu"
    fi
    echo "  Updates:   automatic, from inside Cardfile"
    case ":$PATH:" in
        *:"$BIN_DIR":*) ;;
        *)
            echo ""
            echo "${BLUE}Note: '${BIN_DIR}' is not in your PATH.${NC} Add it to ~/.bashrc or ~/.zshrc:"
            echo "  ${GREEN}export PATH=\"\$PATH:\$HOME/.local/bin\"${NC}"
            ;;
    esac
    echo ""
    echo "Open Cardfile from your application menu, or type: ${GREEN}cardfile${NC}"
}

main() {
    parse_args "$@"
    if [ "$UNINSTALL_MODE" = "true" ]; then
        uninstall
        return 0
    fi
    echo "${BLUE}Cardfile installer for Linux${NC}"
    echo "=============================="
    detect_platform
    check_dependencies

    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM
    TEMP_DIR=$(mktemp -d)

    pick_release
    download_release
    if [ "$VERIFY_ONLY" = "true" ]; then
        echo "${GREEN}Cardfile ${VERSION} downloaded and verified; nothing was installed (--verify-only).${NC}"
        return 0
    fi
    install_files
    install_desktop_integration
    remove_legacy
    check_runtime
    print_summary
}

main "$@"
