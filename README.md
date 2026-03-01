# trebuchet

A application launcher for Hyprland/Wayland.

Built with [iced](https://github.com/iced-rs/iced) and [iced-layershell](https://github.com/waycrate/exwlseat).

## Features

- Full-screen overlay using the Wayland layer-shell protocol
- Real-time fuzzy search across all installed applications
- Icon display from the system icon theme
- Escape to close
- Launches on the active screen

<img width="1009" height="867" alt="image" src="https://github.com/user-attachments/assets/c7145631-065e-4522-856d-e6763bb0f8f0" />

## Requirements

- A Wayland compositor supporting the `wlr-layer-shell` protocol (e.g. Hyprland, Sway)
- Rust toolchain (stable, 2021 edition or later)

## Install

Clone the repository and run the install script:

```sh
git clone https://github.com/rafaelzimmermann/trebuchet.git
cd trebuchet
bash scripts/install.sh
```

This builds a release binary and installs it to `~/.local/bin`. To install system-wide to `/usr/local/bin` instead:

```sh
bash scripts/install.sh --system
```

To also fetch high-resolution icons for ~80 common apps before installing:

```sh
bash scripts/install.sh --icons
```

To uninstall:

```sh
bash scripts/install.sh --uninstall
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
[Papirus on GitHub](https://github.com/PapirusIconTheme/papirus-icon-theme) (GPL-3.0).

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

<a href="https://www.buymeacoffee.com/engzimmermy" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" style="height: 60px !important;width: 217px !important;" ></a>
