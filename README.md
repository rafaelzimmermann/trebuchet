# trebuchet

An application launcher for Hyprland/Wayland.

Built with [iced](https://github.com/iced-rs/iced) and [iced-layershell](https://github.com/waycrate/exwlseat).

## Features

- Screen overlay using the Wayland layer-shell protocol
- Real-time search across all installed applications — type anywhere
- Icon display from the system icon theme (with bundled high-resolution fallbacks)
- Keyboard navigation with arrow keys and Enter to launch
- Terminal apps (`Terminal=true`) are auto-detected and launched in your terminal emulator
- Escape to close
- Launches on the active screen

![trebuchet screenshot](assets/trebuchet-fullview.png)

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
| Escape | Close launcher |

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
