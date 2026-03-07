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
THEMES_DIR="$CONFIG_DIR/themes"

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

# ── Upfront questions (ask everything before the build starts) ────────────────

UPDATE_ICONS=false
if $FETCH_ICONS; then
    if [[ -d assets/icons && -n "$(ls -A assets/icons 2>/dev/null)" ]]; then
        if confirm "Icons already exist. Update them?"; then
            UPDATE_ICONS=true
        fi
    else
        UPDATE_ICONS=true
    fi
fi

# CONFIG_FRESH=true  → fresh install or user chose to overwrite → offer AI setup
# CONFIG_FRESH=false → user kept their existing config → skip AI setup
CONFIG_FRESH=false
OVERWRITE_CONFIG=false
mkdir -p "$CONFIG_DIR"
if [[ ! -f "$CONFIG_FILE" ]]; then
    CONFIG_FRESH=true
else
    if confirm "Config already exists at $CONFIG_FILE. Overwrite?"; then
        OVERWRITE_CONFIG=true
        CONFIG_FRESH=true
    fi
fi

UPDATE_THEMES=false
if [[ ! -d "$THEMES_DIR" || -z "$(ls -A "$THEMES_DIR" 2>/dev/null)" ]]; then
    UPDATE_THEMES=true
elif $OVERWRITE_CONFIG; then
    UPDATE_THEMES=true
else
    if confirm "Themes already exist at $THEMES_DIR. Update them?"; then
        UPDATE_THEMES=true
    fi
fi

# Collect AI setup answers upfront; write to config after install.
DO_AI_SETUP=false
AI_PROVIDER="" AI_MODEL="" AI_API_KEY="" AI_BASE_URL=""

if [[ -t 0 ]] && ! $YES && ! $NO && $CONFIG_FRESH; then
    echo ""
    if confirm "Would you like to set up the AI assistant?"; then
        DO_AI_SETUP=true
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

        local_choice=""
        while true; do
            read -r -n 1 -p "Provider [1-5]: " local_choice
            echo ""
            case "$local_choice" in 1|2|3|4|5) break ;; *) echo "Please enter 1–5." ;; esac
        done

        local_default_model="" local_key_url="" local_needs_key=false local_default_url=""
        case "$local_choice" in
            1) AI_PROVIDER="openai";    local_default_model="gpt-4o";             local_key_url="https://platform.openai.com/api-keys";          local_needs_key=true;  local_default_url="" ;;
            2) AI_PROVIDER="anthropic"; local_default_model="claude-sonnet-4-6";  local_key_url="https://console.anthropic.com/settings/keys";  local_needs_key=true;  local_default_url="" ;;
            3) AI_PROVIDER="gemini";    local_default_model="gemini-2.0-flash";   local_key_url="https://aistudio.google.com/app/apikey";       local_needs_key=true;  local_default_url="" ;;
            4) AI_PROVIDER="ollama";    local_default_model="llama3.2";           local_key_url="";                                             local_needs_key=false; local_default_url="http://localhost:11434" ;;
            5) AI_PROVIDER="openai";    local_default_model="";                   local_key_url="";                                             local_needs_key=true;  local_default_url="" ;;
        esac

        echo ""
        if [[ -n "$local_default_model" ]]; then
            read -r -p "Model [$local_default_model]: " AI_MODEL
            AI_MODEL="${AI_MODEL:-$local_default_model}"
        else
            read -r -p "Model (e.g. gpt-4o, mistral): " AI_MODEL
        fi

        if $local_needs_key; then
            echo ""
            [[ -n "$local_key_url" ]] && echo "Get your API key at: $local_key_url"
            read -r -s -p "API key: " AI_API_KEY
            echo ""
        fi

        echo ""
        if [[ "$local_choice" == "4" ]]; then
            read -r -p "Ollama base URL [$local_default_url]: " AI_BASE_URL
            AI_BASE_URL="${AI_BASE_URL:-$local_default_url}"
        elif [[ "$local_choice" == "5" ]]; then
            read -r -p "Base URL (e.g. http://localhost:8080): " AI_BASE_URL
        fi
    fi
fi

echo ""

# ── Icons ─────────────────────────────────────────────────────────────────────

if $FETCH_ICONS && $UPDATE_ICONS; then
    if [[ -d assets/icons && -n "$(ls -A assets/icons 2>/dev/null)" ]]; then
        echo "Updating icons…"
    else
        echo "Fetching icons…"
    fi
    bash scripts/fetch-icons.sh
elif $FETCH_ICONS; then
    echo "Keeping existing icons."
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

if [[ ! -f "$CONFIG_FILE" ]] || $OVERWRITE_CONFIG; then
    if $OVERWRITE_CONFIG; then
        echo "Config replaced."
    else
        echo "Installing default config to $CONFIG_FILE…"
    fi
    cp assets/trebuchet.conf "$CONFIG_FILE"
else
    echo "Keeping existing config."
fi

if $UPDATE_THEMES && [[ -d assets/themes ]]; then
    echo "Installing themes to $THEMES_DIR…"
    mkdir -p "$THEMES_DIR"
    cp assets/themes/*.conf "$THEMES_DIR/"
else
    echo "Keeping existing themes."
fi

if $DO_AI_SETUP; then
    {
        printf "\n# AI assistant\n"
        printf "ai_provider = %s\n" "$AI_PROVIDER"
        [[ -n "$AI_MODEL"   ]] && printf "ai_model    = %s\n" "$AI_MODEL"
        [[ -n "$AI_API_KEY" ]] && printf "ai_api_key  = %s\n" "$AI_API_KEY"
        [[ -n "$AI_BASE_URL" ]] && printf "ai_base_url = %s\n" "$AI_BASE_URL"
    } >> "$CONFIG_FILE"
    echo ""
    echo "AI assistant configured. Type /ai <question> in trebuchet to try it."
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
[[ -d "$THEMES_DIR" ]] && echo "Themes:     $THEMES_DIR"
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
