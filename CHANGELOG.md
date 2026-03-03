# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- `Terminal=true` desktop entry support: apps that require a terminal are now launched inside an auto-detected terminal emulator (`$TERMINAL`, foot, kitty, alacritty, ghostty, wezterm, xterm)
- `AppEntry::terminal` field parsed from desktop entry files
- File-based configuration: `~/.config/trebuchet/trebuchet.conf` loaded at startup
- `assets/trebuchet.conf` embedded in the binary as the authoritative default config; missing or invalid user keys fall back to it
- Config loading is layered: hardcoded Rust defaults → embedded conf → user conf
- Unit tests for config parsing (missing keys, invalid values, unknown keys, whitespace, comments)
- Updated README: configuration section, keyboard navigation, terminal app support
- `install.sh`: deploy default config to `~/.config/trebuchet/trebuchet.conf` on install; prompt before overwriting an existing one
- `install.sh`: `--uninstall` now removes `~/.config/trebuchet/` in addition to the binary and data files
- `install.sh`: `--yes` / `-y` flag to assume yes for all prompts (non-interactive installs)

## [0.0.1] - 2026-03-02

### Added
- Initial scaffold: trebuchet Wayland app launcher
- Full-screen translucent background with icons-first sort and pagination
- 7×5 grid layout with dot pagination, translucent search bar, fills screen
- Left/right margin; incomplete last row aligned with columns above
- Close on cursor-leave and on unhandled background click
- Exclusive zone set to 0 to respect workspace boundaries
- Compact centred window with rounded corners
- Bundled icon infrastructure with `fetch-icons.sh` script
- Parallelize `fetch-icons.sh` (up to 16 concurrent fetches)
- Embed icons in binary and add install script
- GPL-3.0 license with copyright notice
- Unit tests for exec cleaning, search filter, and pagination
- Type-to-search from anywhere in the window
- Improved fallback icon when no app icon is found
- Keyboard navigation with selection highlighting
- Prompt for sudo password upfront before the build starts

### Fixed
- Compiler warnings; shrink to centered 960×640 popup window
- Icon local lookup: prefer larger sizes before smaller ones
- Icon resolution and grid stability improvements
