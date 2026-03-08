#!/usr/bin/env bash
# Populate assets/icons/ with high-resolution SVG icons for common apps.
#
# Sources (tried in order for each icon):
#   1. Locally installed icon themes (Papirus, Breeze, Adwaita, hicolor …)
#   2. jsDelivr CDN — serves any GitHub repo without needing the branch name
#      https://cdn.jsdelivr.net/gh/PapirusDevelopmentTeam/papirus-icon-theme
#   3. Simple Icons CDN — brand/product SVGs not found in Papirus
#      https://cdn.jsdelivr.net/npm/simple-icons@latest/icons
#   4. Bulk git clone of Papirus as a last resort (one clone, many icons)
#
# Usage:
#   bash scripts/fetch-icons.sh              # writes to assets/icons/, 16 jobs
#   bash scripts/fetch-icons.sh /other/dir   # custom output directory
#   JOBS=4 bash scripts/fetch-icons.sh       # fewer parallel jobs (slow link)

set -euo pipefail

DEST="${1:-assets/icons}"
JOBS="${JOBS:-16}"
mkdir -p "$DEST"

# Temp dir for race-free counters across background subshells.
# CLONE_DIR is also cleaned up here if the script exits abnormally.
TMP=$(mktemp -d)
CLONE_DIR=""
trap 'rm -rf "$TMP" "${CLONE_DIR:-}"' EXIT

# ── Local lookup ──────────────────────────────────────────────────────────────

LOCAL_ROOTS=()
for theme in Papirus breeze hicolor Adwaita AdwaitaLegacy locolor; do
    for base in /usr/share/icons /usr/local/share/icons "$HOME/.local/share/icons"; do
        [[ -d "$base/$theme" ]] && LOCAL_ROOTS+=("$base/$theme")
    done
done

SIZE_DIRS=(scalable 256x256 128x128 96x96 64x64 48x48 32x32)

local_lookup() {
    local name="$1"
    for root in "${LOCAL_ROOTS[@]+"${LOCAL_ROOTS[@]}"}"; do
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

# ── Remote: per-file download via jsDelivr ───────────────────────────────────
#
# jsDelivr resolves the default branch automatically — no branch name needed.
# Fallback to raw.githubusercontent.com with both common branch names.

PAPIRUS_GH="PapirusDevelopmentTeam/papirus-icon-theme"
JSDELIVR="https://cdn.jsdelivr.net/gh/$PAPIRUS_GH"
RAW="https://raw.githubusercontent.com/$PAPIRUS_GH"
REMOTE_SIZES=(64x64 48x48 32x32)

# Papirus uses symlinks extensively — e.g. com.obsproject.Studio.svg contains
# just the text "obs.svg".  CDNs serve the symlink content as-is, so we must
# detect and re-fetch the target, still saving under the originally requested name.
papirus_symlink_target() {
    local file="$1"
    local content
    content=$(cat "$file" 2>/dev/null) || return 1
    # A symlink is a tiny file whose entire content is "something.svg"
    if [[ ${#content} -lt 80 && "$content" =~ ^[a-zA-Z0-9._-]+\.svg$ ]]; then
        echo "$content"
        return 0
    fi
    return 1
}

remote_fetch_one() {
    local lookup="$1" dest="$2"

    for size in "${REMOTE_SIZES[@]}"; do
        # jsDelivr (no branch name required)
        if curl -sf --max-time 15 \
                "$JSDELIVR/Papirus/$size/apps/$lookup.svg" -o "$dest" 2>/dev/null; then
            # Follow Papirus symlink if needed
            if target=$(papirus_symlink_target "$dest"); then
                if curl -sf --max-time 15 \
                        "$JSDELIVR/Papirus/$size/apps/$target" -o "$dest" 2>/dev/null; then
                    return 0
                fi
                rm -f "$dest"
            else
                return 0
            fi
        fi
        # raw.githubusercontent.com fallback (try both common branch names)
        for branch in master main; do
            if curl -sf --max-time 15 \
                    "$RAW/$branch/Papirus/$size/apps/$lookup.svg" -o "$dest" 2>/dev/null; then
                if target=$(papirus_symlink_target "$dest"); then
                    if curl -sf --max-time 15 \
                            "$RAW/$branch/Papirus/$size/apps/$target" -o "$dest" 2>/dev/null; then
                        return 0
                    fi
                    rm -f "$dest"
                else
                    return 0
                fi
            fi
        done
    done
    return 1
}

# ── Remote: Simple Icons CDN (brand icons not in Papirus) ────────────────────

SIMPLE_ICONS_CDN="https://cdn.jsdelivr.net/npm/simple-icons@latest/icons"

remote_fetch_simple_icons() {
    local lookup="$1" dest="$2"
    if curl -sf --max-time 15 \
            "$SIMPLE_ICONS_CDN/$lookup.svg" -o "$dest" 2>/dev/null; then
        return 0
    fi
    return 1
}

# ── Remote: bulk git clone (used when per-file downloads all fail) ────────────
#
# One shallow clone fetches only blobs that are actually needed (sparse checkout),
# then all unresolved icons are copied from the local clone in one pass.

bulk_clone_papirus() {
    [[ -n "$CLONE_DIR" ]] && return 0   # already cloned

    CLONE_DIR=$(mktemp -d)
    echo ""
    echo "  Bulk clone: fetching Papirus icon theme (sparse, depth 1)…"

    if ! git clone --quiet --depth 1 --filter=blob:none --no-checkout \
            "https://github.com/$PAPIRUS_GH.git" \
            "$CLONE_DIR" 2>/dev/null; then
        echo "  ✗  git clone failed — no remote icons available"
        CLONE_DIR=""
        return 1
    fi

    cd "$CLONE_DIR" || { CLONE_DIR=""; return 1; }
    git sparse-checkout init --cone --quiet 2>/dev/null || true
    local sparse_dirs=()
    for size in "${REMOTE_SIZES[@]}"; do sparse_dirs+=("Papirus/$size/apps"); done
    git sparse-checkout set "${sparse_dirs[@]}" --quiet 2>/dev/null || \
        git sparse-checkout set "Papirus" --quiet 2>/dev/null || true
    git checkout --quiet
    cd - >/dev/null
    echo "  Bulk clone ready."
}

remote_fetch_from_clone() {
    local lookup="$1" dest="$2"
    [[ -z "$CLONE_DIR" ]] && return 1
    for size in "${REMOTE_SIZES[@]}"; do
        local src="$CLONE_DIR/Papirus/$size/apps/$lookup.svg"
        if [[ -f "$src" ]]; then
            # Follow Papirus symlink if needed
            if target=$(papirus_symlink_target "$src"); then
                local target_src="$CLONE_DIR/Papirus/$size/apps/$target"
                [[ -f "$target_src" ]] || continue
                src="$target_src"
            fi
            cp "$src" "$dest"
            return 0
        fi
    done
    return 1
}

# ── Icon-name aliases ─────────────────────────────────────────────────────────
# Maps save-name → lookup-name when the Papirus/Simple-Icons filename differs
# from the Icon= field in the .desktop file (and our assets/icons/ filename).

declare -A FETCH_AS=(
    [celluloid]="io.github.celluloid_player.Celluloid"
    [code]="visual-studio-code"
    [epiphany]="org.gnome.Epiphany"
    [gnome-tweaks]="org.gnome.tweaks"
    [handbrake]="fr.handbrake.ghb"
    [org.gnome.Seahorse]="org.gnome.seahorse.Application"
    [pcmanfm]="system-file-manager"
    [totem]="org.gnome.Totem"
)

# ── Manifest generation ───────────────────────────────────────────────────────
# Scans installed .desktop files once to build lookup tables, then writes
# assets/icons/manifest.json for every fetched icon that has non-trivial metadata.

DESKTOP_DIRS=(
    "$HOME/.local/share/applications"
    "/usr/share/applications"
    "/usr/local/share/applications"
)

# Populated by scan_desktop_files() — keyed by Icon= field value.
# Values are tab-separated lists of unique entries.
declare -A DKTOP_NAMES=()   # icon → app display names
declare -A DKTOP_WM=()      # icon → StartupWMClass values

# Single pass over all .desktop files; results stored in DKTOP_NAMES / DKTOP_WM.
scan_desktop_files() {
    for desktop_dir in "${DESKTOP_DIRS[@]}"; do
        [[ -d "$desktop_dir" ]] || continue
        while IFS= read -r -d '' desktop_file; do
            local icon="" name="" wm_class=""
            while IFS='=' read -r key value; do
                value="${value%$'\r'}"
                case "$key" in
                    Icon)           icon="$value" ;;
                    Name)           [[ -z "$name" ]]     && name="$value" ;;
                    StartupWMClass) [[ -z "$wm_class" ]] && wm_class="$value" ;;
                esac
            done < <(grep -E '^(Icon|Name|StartupWMClass)=' "$desktop_file" 2>/dev/null)

            [[ -z "$icon" ]] && continue
            if [[ -n "$name"     && "${DKTOP_NAMES[$icon]:-}" != *"$name"* ]]; then
                DKTOP_NAMES["$icon"]+="${DKTOP_NAMES[$icon]:+$'\t'}$name"
            fi
            if [[ -n "$wm_class" && "${DKTOP_WM[$icon]:-}"   != *"$wm_class"* ]]; then
                DKTOP_WM["$icon"]+="${DKTOP_WM[$icon]:+$'\t'}$wm_class"
            fi
        done < <(find "$desktop_dir" -maxdepth 2 -name '*.desktop' -print0 2>/dev/null)
    done
}

json_str_array() {
    local -n _items=$1
    local first=true
    printf '['
    for item in "${_items[@]}"; do
        [[ "$first" == true ]] && first=false || printf ', '
        printf '"%s"' "${item//\"/\\\"}"
    done
    printf ']'
}

generate_manifest() {
    local out="$DEST/manifest.json"
    local first_entry=true
    local count=0

    scan_desktop_files

    {
        printf '[\n'
        for name in "${ICONS[@]}"; do
            local file=""
            [[ -f "$DEST/$name.svg" ]] && file="$name.svg"
            [[ -z "$file" && -f "$DEST/$name.png" ]] && file="$name.png"
            [[ -z "$file" ]] && continue

            # icon_name: save-name + FETCH_AS alias (if different)
            local -a icon_names=("$name")
            local fetch_alias="${FETCH_AS[$name]:-}"
            [[ -n "$fetch_alias" && "$fetch_alias" != "$name" ]] && icon_names+=("$fetch_alias")

            # Gather metadata via O(1) table lookup for each alias
            local -a app_names=() wm_classes=()
            for lookup in "${icon_names[@]}"; do
                if [[ -n "${DKTOP_NAMES[$lookup]:-}" ]]; then
                    local -a _n; IFS=$'\t' read -r -a _n <<< "${DKTOP_NAMES[$lookup]}"
                    for n in "${_n[@]}"; do
                        local dup=false
                        for e in "${app_names[@]+"${app_names[@]}"}"; do [[ "$e" == "$n" ]] && dup=true && break; done
                        $dup || app_names+=("$n")
                    done
                fi
                if [[ -n "${DKTOP_WM[$lookup]:-}" ]]; then
                    local -a _w; IFS=$'\t' read -r -a _w <<< "${DKTOP_WM[$lookup]}"
                    for w in "${_w[@]}"; do
                        local dup=false
                        for e in "${wm_classes[@]+"${wm_classes[@]}"}"; do [[ "$e" == "$w" ]] && dup=true && break; done
                        $dup || wm_classes+=("$w")
                    done
                fi
            done

            [[ "$first_entry" == true ]] && first_entry=false || printf ',\n'
            printf '  {\n'
            printf '    "file": "%s"' "$file"
            [[ ${#wm_classes[@]} -gt 0 ]] && { printf ',\n    "wm_class": '; json_str_array wm_classes; }
            printf ',\n    "icon_name": '; json_str_array icon_names
            [[ ${#app_names[@]}  -gt 0 ]] && { printf ',\n    "app_name": ';  json_str_array app_names; }
            printf '\n  }'
            (( count++ )) || true
        done
        printf '\n]\n'
    } > "$out"

    echo "Manifest: $out  ($count entries)"
}

# ── Per-icon fetch orchestration ──────────────────────────────────────────────

fetch() {
    local name="$1"
    local lookup="${FETCH_AS[$name]:-$name}"
    local dest="$DEST/$name.svg"

    if [[ -f "$dest" || -f "$DEST/$name.png" ]]; then
        touch "$TMP/skip_${name}"
        return 0
    fi

    local src
    if src=$(local_lookup "$lookup" 2>/dev/null); then
        cp "$src" "$dest"
        echo "  ✓  $name"
        touch "$TMP/ok_${name}"
        return 0
    fi

    if remote_fetch_one "$lookup" "$dest"; then
        [[ "$lookup" != "$name" ]] \
            && echo "  ↓  $name  (← $lookup)" \
            || echo "  ↓  $name"
        touch "$TMP/ok_${name}"
        return 0
    fi

    if remote_fetch_simple_icons "$lookup" "$dest"; then
        echo "  ↓  $name  (simple-icons)"
        touch "$TMP/ok_${name}"
        return 0
    fi

    # Mark for bulk-clone pass.
    touch "$TMP/need_clone_${name}"
}

# ── App list ──────────────────────────────────────────────────────────────────

ICONS=(
    # Browsers
    firefox chromium google-chrome brave-browser microsoft-edge
    opera vivaldi epiphany librewolf

    # Terminals
    org.gnome.Terminal kitty alacritty konsole tilix
    wezterm foot com.mitchellh.ghostty xterm

    # File managers
    org.gnome.Nautilus thunar dolphin nemo pcmanfm

    # Editors & IDEs
    code code-oss vscodium sublime-text emacs
    nvim gedit org.gnome.TextEditor org.kde.kate helix

    # Communication
    discord slack signal-desktop telegram-desktop thunderbird
    zoom teams element-desktop hexchat skypeforlinux
    whatsapp 
    
    # AI / LLM services
    anthropic claude openai googlegemini mistralai
    perplexity huggingface ollama

    # Media
    spotify vlc mpv celluloid rhythmbox clementine lollypop
    audacious totem com.obsproject.Studio
    handbrake shotwell darktable rawtherapee

    # Productivity
    libreoffice-writer libreoffice-calc libreoffice-impress
    libreoffice-draw libreoffice-base obsidian typora ghostwriter marktext

    # Design & Graphics
    gimp inkscape krita blender scribus digikam shotcut kdenlive

    # Development
    gitg gitkraken dbeaver postman insomnia
    beekeeper-studio virtualbox gnome-boxes

    # Gaming
    steam lutris heroic

    # System utilities
    org.gnome.Settings gnome-control-center org.gnome.Calculator
    org.gnome.SystemMonitor org.gnome.DiskUtility org.gnome.baobab
    gparted keepassxc bitwarden org.gnome.Seahorse gnome-tweaks
    org.gnome.Extensions synaptic

    # Misc
    evince org.gnome.Evince okular org.gnome.clocks org.gnome.Maps
    org.gnome.Calendar org.gnome.Contacts transmission-gtk
    qbittorrent filezilla remmina
)

# ── Pass 1: parallel per-file fetches ─────────────────────────────────────────

total=${#ICONS[@]}
echo "Fetching $total icons (up to $JOBS in parallel) …"
echo ""

for name in "${ICONS[@]}"; do
    fetch "$name" &
    while (( $(jobs -r | wc -l) >= JOBS )); do
        wait -n 2>/dev/null || sleep 0.05
    done
done
wait

# ── Pass 2: bulk git clone for anything still missing ─────────────────────────

need_clone=( "$TMP"/need_clone_* )
if [[ -e "${need_clone[0]}" ]]; then
    bulk_clone_papirus || true   # CLONE_DIR="" on failure; handled below
    for marker in "${need_clone[@]}"; do
        name="${marker##*/need_clone_}"
        lookup="${FETCH_AS[$name]:-$name}"
        dest="$DEST/$name.svg"
        if remote_fetch_from_clone "$lookup" "$dest"; then
            [[ "$lookup" != "$name" ]] \
                && echo "  ↓  $name  (← $lookup, from clone)" \
                || echo "  ↓  $name  (from clone)"
            rm "$marker"
            touch "$TMP/ok_${name}"
        else
            echo "  ✗  $name"
            touch "$TMP/fail_${name}"
        fi
    done
fi

# ── Summary ───────────────────────────────────────────────────────────────────

ok=$(find   "$TMP" -name "ok_*"   | wc -l)
skip=$(find "$TMP" -name "skip_*" | wc -l)
fail=$(find "$TMP" -name "fail_*" | wc -l)

echo ""
echo "Done.  ✓ $ok fetched   = $skip already existed   ✗ $fail not found"
echo "Total: $(find "$DEST" \( -name '*.svg' -o -name '*.png' \) | wc -l) icons in $DEST"

# ── Generate manifest ──────────────────────────────────────────────────────────
generate_manifest
