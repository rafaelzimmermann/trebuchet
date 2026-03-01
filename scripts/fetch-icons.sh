#!/usr/bin/env bash
# Populate assets/icons/ with high-resolution SVG icons for common apps.
#
# Strategy (in priority order):
#   1. Local icon themes already installed on this machine
#      (Papirus, Breeze, Adwaita, hicolor — searched from largest to smallest)
#   2. Download from Papirus icon theme on GitHub (requires internet)
#      https://github.com/PapirusIconTheme/papirus-icon-theme  GPL-3.0
#
# Usage:
#   bash scripts/fetch-icons.sh            # writes to assets/icons/
#   bash scripts/fetch-icons.sh /some/dir  # custom output directory

set -euo pipefail

DEST="${1:-assets/icons}"
mkdir -p "$DEST"

ok=0; skip=0; fail=0

# ── Icon resolution helpers ───────────────────────────────────────────────────

# Ordered list of local theme search roots (prefer larger sizes).
LOCAL_ROOTS=()
for theme in Papirus breeze hicolor Adwaita AdwaitaLegacy locolor; do
    for base in /usr/share/icons /usr/local/share/icons "$HOME/.local/share/icons"; do
        [[ -d "$base/$theme" ]] && LOCAL_ROOTS+=("$base/$theme")
    done
done

# Preferred size folders, largest first.
SIZE_DIRS=(scalable 96x96 64x64 48x48 symbolic 32x32)

local_lookup() {
    local name="$1"
    for root in "${LOCAL_ROOTS[@]}"; do
        for sz in "${SIZE_DIRS[@]}"; do
            for subdir in apps categories; do
                for ext in svg png; do
                    local p="$root/$sz/$subdir/$name.$ext"
                    [[ -f "$p" ]] && { echo "$p"; return 0; }
                done
            done
        done
    done
    return 1
}

PAPIRUS_BRANCHES=(master main)
PAPIRUS_BASE="https://raw.githubusercontent.com/PapirusIconTheme/papirus-icon-theme"

remote_fetch() {
    local name="$1"
    local dest="$DEST/$name.svg"
    for branch in "${PAPIRUS_BRANCHES[@]}"; do
        for size in 64x64 48x48 32x32; do
            local url="$PAPIRUS_BASE/$branch/Papirus/$size/apps/$name.svg"
            if curl -sf --max-time 10 "$url" -o "$dest" 2>/dev/null; then
                echo "  ↓  $name  (remote $size/$branch)"
                return 0
            fi
        done
    done
    return 1
}

fetch() {
    local name="$1"
    local dest_svg="$DEST/$name.svg"
    local dest_png="$DEST/$name.png"

    # Already have it.
    if [[ -f "$dest_svg" || -f "$dest_png" ]]; then
        (( skip++ )) || true
        return 0
    fi

    # Try local theme first.
    local src
    if src=$(local_lookup "$name" 2>/dev/null); then
        local ext="${src##*.}"
        cp "$src" "$DEST/$name.$ext"
        echo "  ✓  $name  (local: ${src#$HOME})"
        (( ok++ )) || true
        return 0
    fi

    # Fall back to remote download.
    if remote_fetch "$name"; then
        (( ok++ )) || true
        return 0
    fi

    echo "  ✗  $name"
    (( fail++ )) || true
    return 0  # non-fatal
}

# ── App list ──────────────────────────────────────────────────────────────────

echo "── Browsers ──────────────────────────"
fetch firefox
fetch chromium
fetch google-chrome
fetch brave-browser
fetch microsoft-edge
fetch opera
fetch vivaldi
fetch epiphany
fetch librewolf

echo "── Terminals ─────────────────────────"
fetch org.gnome.Terminal
fetch kitty
fetch alacritty
fetch konsole
fetch tilix
fetch wezterm
fetch foot
fetch ghostty
fetch xterm

echo "── File managers ─────────────────────"
fetch org.gnome.Nautilus
fetch thunar
fetch dolphin
fetch nemo
fetch pcmanfm

echo "── Editors & IDEs ────────────────────"
fetch code
fetch code-oss
fetch vscodium
fetch sublime-text
fetch emacs
fetch nvim
fetch gedit
fetch org.gnome.TextEditor
fetch org.kde.kate
fetch helix

echo "── Communication ─────────────────────"
fetch discord
fetch slack
fetch signal-desktop
fetch telegram-desktop
fetch thunderbird
fetch zoom
fetch teams
fetch element-desktop
fetch hexchat
fetch skypeforlinux

echo "── Media ─────────────────────────────"
fetch spotify
fetch vlc
fetch mpv
fetch celluloid
fetch rhythmbox
fetch clementine
fetch lollypop
fetch audacious
fetch totem
fetch obs-studio
fetch com.obsproject.Studio
fetch handbrake
fetch shotwell
fetch darktable
fetch rawtherapee

echo "── Productivity ──────────────────────"
fetch libreoffice-writer
fetch libreoffice-calc
fetch libreoffice-impress
fetch libreoffice-draw
fetch libreoffice-base
fetch obsidian
fetch typora
fetch ghostwriter
fetch marktext

echo "── Design & Graphics ─────────────────"
fetch gimp
fetch inkscape
fetch krita
fetch blender
fetch scribus
fetch digikam
fetch shotcut
fetch kdenlive

echo "── Development ───────────────────────"
fetch gitg
fetch gitkraken
fetch dbeaver
fetch postman
fetch insomnia
fetch beekeeper-studio
fetch virtualbox
fetch gnome-boxes

echo "── Gaming ────────────────────────────"
fetch steam
fetch lutris
fetch heroic

echo "── System utilities ──────────────────"
fetch org.gnome.Settings
fetch gnome-control-center
fetch org.gnome.Calculator
fetch org.gnome.SystemMonitor
fetch org.gnome.DiskUtility
fetch org.gnome.baobab
fetch gparted
fetch keepassxc
fetch bitwarden
fetch org.gnome.Seahorse
fetch gnome-tweaks
fetch org.gnome.Extensions
fetch synaptic
fetch pamac-manager

echo "── Misc ──────────────────────────────"
fetch evince
fetch org.gnome.Evince
fetch okular
fetch org.gnome.clocks
fetch org.gnome.Maps
fetch org.gnome.Calendar
fetch org.gnome.Contacts
fetch transmission-gtk
fetch qbittorrent
fetch filezilla
fetch remmina

echo ""
echo "Done.  ✓ $ok fetched   = $skip already existed   ✗ $fail not found"
echo "Output: $DEST  ($(find "$DEST" -name '*.svg' -o -name '*.png' | wc -l) icons)"
