#!/usr/bin/env bash
# Install trebuchet — Wayland app launcher
#
# One-line install (recommended):
#   sh -c "$(curl -fsSL https://raw.githubusercontent.com/rafaelzimmermann/trebuchet/main/scripts/install.sh)"
#
# Installs system-wide to /usr/local/bin with high-resolution icons by default.
#
# Options:
#   --user        install to ~/.local/bin instead of /usr/local/bin
#   --no-icons    skip fetching high-resolution icons
#   --uninstall   remove installed files
#   --yes         assume yes for all prompts (non-interactive)
#
# When run from the project root (where Cargo.toml lives), the local source is used.
# Otherwise the repository is cloned automatically.

set -euo pipefail

# ── Argument parsing ──────────────────────────────────────────────────────────

SYSTEM=true
FETCH_ICONS=true
UNINSTALL=false
YES=false

for arg in "$@"; do
    case "$arg" in
        --user)      SYSTEM=false ;;
        --no-icons)  FETCH_ICONS=false ;;
        --uninstall) UNINSTALL=true ;;
        --yes|-y)    YES=true ;;
        # Legacy flags (now the default — kept for compatibility)
        --system)    SYSTEM=true ;;
        --icons)     FETCH_ICONS=true ;;
        --help|-h)
            sed -n '2,15p' "$0" | sed 's/^# \?//'
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
CONFIG_DIR="${HOME}/.config/trebuchet"
CONFIG_FILE="$CONFIG_DIR/trebuchet.conf"

# ── Privilege helpers ─────────────────────────────────────────────────────────
# Use sudo only when --system and we're not already root.
# Prompt upfront so the password isn't requested mid-build.

if $SYSTEM && [[ $EUID -ne 0 ]]; then
    PRIV="sudo"
    echo "sudo access is required for system-wide install."
    sudo -v
    # Keep the sudo credential alive in the background for the duration of
    # the build (which can take several minutes on a fresh checkout).
    ( while true; do sudo -n true; sleep 50; done ) &
    SUDO_KEEPALIVE_PID=$!
    trap 'kill "$SUDO_KEEPALIVE_PID" 2>/dev/null' EXIT
else
    PRIV=""
fi

priv_mkdir() { $PRIV mkdir -p "$@"; }
priv_install() { $PRIV install "$@"; }
priv_cp() { $PRIV cp "$@"; }
priv_tee() { $PRIV tee "$@" >/dev/null; }
priv_rm() { $PRIV rm "$@"; }

# ── Uninstall ─────────────────────────────────────────────────────────────────

if $UNINSTALL; then
    echo "Uninstalling trebuchet…"
    priv_rm -f  "$BINARY"
    priv_rm -f  "$DESKTOP_FILE"
    priv_rm -rf "$DATA_DIR"
    rm -rf "$CONFIG_DIR"
    echo "Done."
    exit 0
fi

# ── Source (clone if not in project root) ─────────────────────────────────────

if [[ ! -f Cargo.toml ]]; then
    if ! command -v git &>/dev/null; then
        echo "Error: git is required to install trebuchet." >&2
        exit 1
    fi
    CLONE_DIR=$(mktemp -d)
    trap 'rm -rf "$CLONE_DIR"' EXIT
    echo "Cloning trebuchet…"
    git clone --depth=1 https://github.com/rafaelzimmermann/trebuchet.git "$CLONE_DIR"
    cd "$CLONE_DIR"
fi

# ── Icons ─────────────────────────────────────────────────────────────────────

if $FETCH_ICONS; then
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

# Install default config; prompt before overwriting an existing one.
mkdir -p "$CONFIG_DIR"
if [[ ! -f "$CONFIG_FILE" ]]; then
    echo "Installing default config to $CONFIG_FILE…"
    cp assets/trebuchet.conf "$CONFIG_FILE"
else
    if $YES; then
        reply="y"
    else
        read -r -p "Config already exists at $CONFIG_FILE. Overwrite? [y/N] " reply
    fi
    if [[ "${reply,,}" == "y" ]]; then
        cp assets/trebuchet.conf "$CONFIG_FILE"
        echo "Config replaced."
    else
        echo "Keeping existing config."
    fi
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
echo "Config:     $CONFIG_FILE"
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
