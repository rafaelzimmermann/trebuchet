# trebuchet

An application launcher for Hyprland/Wayland.

Built with [iced](https://github.com/iced-rs/iced) and [iced-layershell](https://github.com/waycrate/exwlseat).


https://github.com/user-attachments/assets/117d5e0b-9064-4bc3-b002-a6ce9533ec2c



## Features

- Screen overlay using the Wayland layer-shell protocol
- Real-time search across all installed applications — type anywhere
- Icon display from the system icon theme (with bundled high-resolution fallbacks)
- Keyboard navigation with arrow keys and Enter to launch
- Terminal apps (`Terminal=true`) are auto-detected and launched in your terminal emulator
- Escape to close
- Launches on the active screen
- Window mover — type `/mv` to pull a window from another workspace into the current one
- Custom commands — define shell shortcuts in config (e.g. `/shutdown`, `/uptime`) with optional output display


## Requirements

- A Wayland compositor supporting the `wlr-layer-shell` protocol (e.g. Hyprland, Sway)
- Rust toolchain (stable, 2021 edition or later)

## Install

```sh
sh -c "$(curl -fsSL https://raw.githubusercontent.com/rafaelzimmermann/trebuchet/main/scripts/install.sh)"
```

This clones the repository, fetches high-resolution icons for ~80 common apps, builds a release binary, and installs it system-wide to `/usr/local/bin`.

### Options

| Flag | Description |
|------|-------------|
| `--user` | Install to `~/.local/bin` instead of `/usr/local/bin` |
| `--no-icons` | Skip fetching high-resolution icons |
| `--uninstall` | Remove installed files |

Pass flags by cloning and running the script directly:

```sh
git clone --depth=1 https://github.com/rafaelzimmermann/trebuchet.git
bash trebuchet/scripts/install.sh --user
```

To uninstall:

```sh
bash trebuchet/scripts/install.sh --uninstall
```

### Bind to a key

Add this to your Hyprland config (`~/.config/hypr/hyprland.conf`):

```
bind = SUPER, Space, exec, trebuchet
```

## Setup from source

### 1. Fetch bundled icons (optional)

trebuchet ships a script that populates `assets/icons/` with high-resolution SVGs
for ~80 common applications. It checks locally installed icon themes first
(Papirus, Breeze, hicolor …) and falls back to downloading from
[Papirus on GitHub](https://github.com/PapirusDevelopmentTeam/papirus-icon-theme) (GPL-3.0).

```sh
bash scripts/fetch-icons.sh
```

These icons take priority over the system icon theme at runtime, so lower-resolution
or missing system icons are automatically covered. The fetched files are excluded from
version control (see `.gitignore`).

The script also generates `assets/icons/manifest.json` by scanning your installed
`.desktop` files. The manifest records icon aliases (`wm_class`, `icon_name`, `app_name`)
so the window mover can match running windows to their icons even when the WM class
doesn't match the icon filename (e.g. VS Code's WM class `Code` → `code.svg`).

If you have Papirus installed (`pacman -S papirus-icon-theme` / `apt install papirus-icon-theme`),
the script works entirely offline.

### 2. Build and run

```sh
cargo run --release
```

## Usage

| Action | Effect |
|--------|--------|
| Type | Filter applications by name |
| Arrow keys | Move selection through the grid |
| Enter | Launch selected application |
| Click | Launch application |
| `/mv` + Space or Enter | Open window mover |
| `/cmd` + Space or Enter | Open custom command runner |
| `/config` + Space or Enter | Open settings panel |
| `/app` + Space or Enter | Return to app grid from any panel |
| Escape | Return to app grid (from any panel) |

## Window mover

Type `/mv` (then Space or Enter) to switch to the window mover. It shows all open windows on other workspaces in the same grid layout as the app launcher.

```
/mv          → show all windows on other workspaces
/mv fire     → show windows matching "fire" (filters by title and class)
```

Each cell shows the app icon and a `workspace:title` label. As you hover or navigate with the keyboard the full label is shown at the bottom of the panel.

Selecting a window with Enter or a click moves it silently to your current workspace and closes the launcher. **Escape** returns to the app grid without moving anything.

Windows are ordered by workspace ID, then left-to-right by position within each workspace.

## Settings panel

Type `/config` (then Space or Enter) to open the settings panel. From there you can switch the colour theme:

```
theme <name>
```

Trebuchet looks for `.conf` files in `~/.config/trebuchet/themes/`. The panel lists all available themes at idle so you can see your options at a glance.

```sh
# Example: drop a theme file into place
cp my-theme.conf ~/.config/trebuchet/themes/my-theme.conf
# Then inside trebuchet:
# /config → theme my-theme
```

The Copy button copies the last command output (prompt + result) to the clipboard.

## Custom commands

Define shell shortcuts that trigger by typing a prefix and pressing Enter.

```ini
# ~/.config/trebuchet/trebuchet.conf

[[command]]
prefix  = /shutdown
command = shutdown -h now

[[command]]
prefix  = /reboot
command = reboot
```

Set `display_result = true` to capture stdout and show it in the response panel instead of closing the launcher:

```ini
[[command]]
prefix         = /uptime
command        = uptime -p
display_result = true

[[command]]
prefix         = /ip
command        = ip -br addr show
display_result = true
```

The `command` is executed with `sh -c`, so pipes, substitutions, and any shell built-in work. Multiple `[[command]]` blocks can be defined; they accumulate across config layers.

### Using the command runner

Type `/cmd` (then Space or Enter) to open the command runner panel. The idle view lists all configured prefixes. Type a prefix and press Enter to run it:

- Commands with `display_result = false` (default) run silently and close the launcher.
- Commands with `display_result = true` run asynchronously — a "Running…" indicator appears immediately; the output is shown in the panel when the command completes. The Copy button copies the prompt + output to the clipboard.

## Configuration

trebuchet reads `~/.config/trebuchet/trebuchet.conf` on startup. If the file does not
exist or a setting is missing, the built-in defaults apply.

```ini
# ~/.config/trebuchet/trebuchet.conf

columns   = 7
rows      = 5
icon_size = 96
```

| Setting | Default | Description |
|---------|---------|-------------|
| `columns` | `7` | Number of app columns in the grid |
| `rows` | `5` | Number of app rows per page |
| `icon_size` | `96` | Icon size in pixels |

---

<a href="https://www.buymeacoffee.com/engzimmermy" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" style="height: 60px !important;width: 217px !important;" ></a>
