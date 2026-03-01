# trebuchet

A macOS Launchpad-style full-screen application launcher for Hyprland/Wayland.

Built with [iced](https://github.com/iced-rs/iced) and [iced-layershell](https://github.com/waycrate/exwlseat).

## Features

- Full-screen overlay using the Wayland layer-shell protocol
- Real-time fuzzy search across all installed applications
- Icon display from the system icon theme
- Escape to close
- Launches on the active screen

## Requirements

- A Wayland compositor supporting the `wlr-layer-shell` protocol (e.g. Hyprland, Sway)
- Rust toolchain (stable, 2021 edition or later)

## Setup

### 1. Fetch bundled icons (recommended)

trebuchet ships a script that populates `assets/icons/` with high-resolution SVGs
for ~80 common applications. It checks locally installed icon themes first
(Papirus, Breeze, hicolor …) and falls back to downloading from
[Papirus on GitHub](https://github.com/PapirusIconTheme/papirus-icon-theme) (GPL-3.0).

```sh
bash scripts/fetch-icons.sh
```

These icons take priority over the system icon theme at runtime, so lower-resolution
or missing system icons are automatically covered. The fetched files are excluded from
version control (see `.gitignore`).

If you have Papirus installed (`pacman -S papirus-icon-theme` / `apt install papirus-icon-theme`),
the script works entirely offline.

### 2. Build

```sh
cargo build --release
```

## Run

```sh
cargo run --release
```

Or after building, copy the binary to your PATH:

```sh
cp target/release/trebuchet ~/.local/bin/
```

Then bind it to a key in your Hyprland config:

```
bind = SUPER, Space, exec, trebuchet
```

## Usage

| Action | Effect |
|--------|--------|
| Type   | Filter applications by name |
| Click  | Launch application |
| Escape | Close launcher |

## Configuration

Configuration is currently baked in via `src/config.rs`. Default values:

| Setting | Default | Description |
|---------|---------|-------------|
| `columns` | `6` | Number of app columns in the grid |
| `icon_size` | `96` | Icon size in pixels |
| `background_opacity` | `0.85` | Background opacity (0.0–1.0) |

---

<a href="https://www.buymeacoffee.com/engzimmermy"><img src="https://img.buymeacoffee.com/button-api/?text=Buy me a coffee&emoji=&slug=engzimmermy&button_colour=FFDD00&font_colour=000000&font_family=Cookie&outline_colour=000000&coffee_colour=ffffff" /></a>
