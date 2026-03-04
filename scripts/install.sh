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
#   --no          assume no for all prompts (non-interactive)
#
# When run from the project root (where Cargo.toml lives), the local source is used.
# Otherwise the repository is cloned automatically.

set -euo pipefail

# ── Argument parsing ──────────────────────────────────────────────────────────

SYSTEM=true
FETCH_ICONS=true
UNINSTALL=false
YES=false
NO=false

for arg in "$@"; do
    case "$arg" in
        --user)      SYSTEM=false ;;
        --no-icons)  FETCH_ICONS=false ;;
        --uninstall) UNINSTALL=true ;;
        --yes|-y)    YES=true ;;
        --no|-n)     NO=true ;;
        # Legacy flags (now the default — kept for compatibility)
        --system)    SYSTEM=true ;;
        --icons)     FETCH_ICONS=true ;;
        --help|-h)
            sed -n '2,16p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "Unknown option: $arg"
            echo "Run with --help to see available options."
            exit 1
            ;;
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

# confirm <prompt>  — returns 0 (yes) or 1 (no).
# With --yes, always returns 0; with --no, always returns 1 without prompting.
confirm() {
    if $YES; then return 0; fi
    if $NO;  then return 1; fi
    local reply
    read -r -n 1 -p "$1 [y/N] " reply
    echo ""
    [[ "${reply,,}" == "y" ]]
}

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
    if [[ -d assets/icons && -n "$(ls -A assets/icons 2>/dev/null)" ]]; then
        if confirm "Icons already exist. Update them?"; then
            echo "Updating icons…"
            bash scripts/fetch-icons.sh
        else
            echo "Keeping existing icons."
        fi
    else
        echo "Fetching icons…"
        bash scripts/fetch-icons.sh
    fi
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
# CONFIG_FRESH=true  → fresh install or user chose to overwrite → offer AI setup
# CONFIG_FRESH=false → user kept their existing config → skip AI setup
CONFIG_FRESH=false
mkdir -p "$CONFIG_DIR"
if [[ ! -f "$CONFIG_FILE" ]]; then
    echo "Installing default config to $CONFIG_FILE…"
    cp assets/trebuchet.conf "$CONFIG_FILE"
    CONFIG_FRESH=true
else
    if confirm "Config already exists at $CONFIG_FILE. Overwrite?"; then
        cp assets/trebuchet.conf "$CONFIG_FILE"
        echo "Config replaced."
        CONFIG_FRESH=true
    else
        echo "Keeping existing config."
    fi
fi

# ── AI assistant setup ────────────────────────────────────────────────────────
# Offered only in an interactive terminal; skipped when --yes is set.

setup_ai() {
    echo ""
    echo "AI assistant setup"
    echo "════════════════════════════════════════════"
    echo "Enables '/ai <question>' in the launcher."
    echo ""
    echo "  1) OpenAI     — default: gpt-4o             (API key required)"
    echo "  2) Anthropic  — default: claude-sonnet-4-6  (API key required)"
    echo "  3) Gemini     — default: gemini-2.0-flash   (API key required)"
    echo "  4) Ollama     — local, no API key needed"
    echo "  5) Other      — custom OpenAI-compatible endpoint"
    echo ""

    local choice
    while true; do
        read -r -n 1 -p "Provider [1-5]: " choice
        echo ""
        case "$choice" in 1|2|3|4|5) break ;; *) echo "Please enter 1–5." ;; esac
    done

    local provider default_model key_url needs_key default_url
    case "$choice" in
        1) provider="openai";    default_model="gpt-4o";             key_url="https://platform.openai.com/api-keys";          needs_key=true;  default_url="" ;;
        2) provider="anthropic"; default_model="claude-sonnet-4-6";  key_url="https://console.anthropic.com/settings/keys";  needs_key=true;  default_url="" ;;
        3) provider="gemini";    default_model="gemini-2.0-flash";   key_url="https://aistudio.google.com/app/apikey";       needs_key=true;  default_url="" ;;
        4) provider="ollama";    default_model="llama3.2";           key_url="";                                             needs_key=false; default_url="http://localhost:11434" ;;
        5) provider="openai";    default_model="";                   key_url="";                                             needs_key=true;  default_url="" ;;
    esac

    # Model
    echo ""
    local model
    if [[ -n "$default_model" ]]; then
        read -r -p "Model [$default_model]: " model
        model="${model:-$default_model}"
    else
        read -r -p "Model (e.g. gpt-4o, mistral): " model
    fi

    # API key
    local api_key=""
    if $needs_key; then
        echo ""
        [[ -n "$key_url" ]] && echo "Get your API key at: $key_url"
        read -r -s -p "API key: " api_key
        echo ""
    fi

    # Base URL — prompted for Ollama (to allow overriding) and Other (required)
    local base_url=""
    echo ""
    if [[ "$choice" == "4" ]]; then
        read -r -p "Ollama base URL [$default_url]: " base_url
        base_url="${base_url:-$default_url}"
    elif [[ "$choice" == "5" ]]; then
        read -r -p "Base URL (e.g. http://localhost:8080): " base_url
    fi

    # Append settings to config
    {
        printf "\n# AI assistant\n"
        printf "ai_provider = %s\n" "$provider"
        [[ -n "$model"    ]] && printf "ai_model    = %s\n" "$model"
        [[ -n "$api_key"  ]] && printf "ai_api_key  = %s\n" "$api_key"
        [[ -n "$base_url" ]] && printf "ai_base_url = %s\n" "$base_url"
    } >> "$CONFIG_FILE"

    echo ""
    echo "AI assistant configured."
    echo "Type /ai <question> in trebuchet to try it."
}

if [[ -t 0 ]] && ! $YES && ! $NO && $CONFIG_FRESH; then
    echo ""
    if confirm "Would you like to set up the AI assistant?"; then
        setup_ai
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
