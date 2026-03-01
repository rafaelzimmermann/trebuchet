#!/usr/bin/env bash
# Install trebuchet — Wayland app launcher
#
# Usage:
#   bash scripts/install.sh              # user install → ~/.local/bin
#   bash scripts/install.sh --system     # system install → /usr/local/bin  (uses sudo for copies)
#   bash scripts/install.sh --icons      # fetch icons before installing
#   bash scripts/install.sh --uninstall  # remove installed files
#
# Must be run from the project root (where Cargo.toml lives).
# cargo always runs as YOU — sudo is only used for the file copies with --system.

set -euo pipefail

# ── Argument parsing ──────────────────────────────────────────────────────────

SYSTEM=false
FETCH_ICONS=false
UNINSTALL=false

for arg in "$@"; do
    case "$arg" in
        --system)    SYSTEM=true ;;
        --icons)     FETCH_ICONS=true ;;
        --uninstall) UNINSTALL=true ;;
        --help|-h)
            sed -n '2,10p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *) echo "Unknown option: $arg"; exit 1 ;;
    esac
done

# ── Paths ─────────────────────────────────────────────────────────────────────

if $SYSTEM; then
    BIN_DIR="/usr/local/bin"
    DATA_DIR="/usr/local/share/trebuchet"
    DESKTOP_DIR="/usr/local/share/applications"
else
    BIN_DIR="${HOME}/.local/bin"
    DATA_DIR="${HOME}/.local/share/trebuchet"
    DESKTOP_DIR="${HOME}/.local/share/applications"
fi

BINARY="$BIN_DIR/trebuchet"
ICON_DEST="$DATA_DIR/icons"
DESKTOP_FILE="$DESKTOP_DIR/trebuchet.desktop"

# ── Privilege helpers ─────────────────────────────────────────────────────────
# Use sudo only when --system and we're not already root.

if $SYSTEM && [[ $EUID -ne 0 ]]; then
    PRIV="sudo"
else
    PRIV=""
fi

priv_mkdir() { $PRIV mkdir -p "$@"; }
priv_install() { $PRIV install "$@"; }
priv_cp() { $PRIV cp "$@"; }
priv_tee() { $PRIV tee "$@" >/dev/null; }
priv_rm() { $PRIV rm "$@"; }

# ── Sanity check ──────────────────────────────────────────────────────────────

if [[ ! -f Cargo.toml ]]; then
    echo "Error: run this script from the project root (where Cargo.toml lives)." >&2
    exit 1
fi

# ── Uninstall ─────────────────────────────────────────────────────────────────

if $UNINSTALL; then
    echo "Uninstalling trebuchet…"
    priv_rm -f  "$BINARY"
    priv_rm -f  "$DESKTOP_FILE"
    priv_rm -rf "$DATA_DIR"
    echo "Done."
    exit 0
fi

# ── Icons ─────────────────────────────────────────────────────────────────────

if $FETCH_ICONS || [[ ! -d assets/icons || -z "$(ls -A assets/icons 2>/dev/null)" ]]; then
    echo "Fetching icons…"
    bash scripts/fetch-icons.sh
fi

# ── Build (always as the invoking user) ───────────────────────────────────────

echo "Building trebuchet (release)…"
cargo build --release

# ── Install ───────────────────────────────────────────────────────────────────

echo "Installing to $BIN_DIR…"
priv_mkdir "$BIN_DIR"
priv_install -m 755 target/release/trebuchet "$BINARY"

if [[ -d assets/icons && -n "$(ls -A assets/icons 2>/dev/null)" ]]; then
    echo "Installing icons to $ICON_DEST…"
    priv_mkdir "$ICON_DEST"
    priv_cp -r assets/icons/. "$ICON_DEST/"
fi

priv_mkdir "$DESKTOP_DIR"
priv_tee "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Trebuchet
Comment=Wayland full-screen app launcher
Exec=trebuchet
Categories=Utility;
NoDisplay=true
EOF

echo ""
echo "Installed:  $BINARY"
[[ -d "$ICON_DEST" ]] && echo "Icons:      $ICON_DEST"
echo "Desktop:    $DESKTOP_FILE"
echo ""

# Warn if the bin dir isn't on PATH.
if ! command -v trebuchet &>/dev/null 2>&1; then
    echo "Note: $BIN_DIR is not on your PATH."
    echo "Add it to your shell profile:"
    echo "  export PATH=\"\$PATH:$BIN_DIR\""
    echo ""
fi

echo "Bind it to a key in your Hyprland config:"
echo "  bind = SUPER, Space, exec, trebuchet"
