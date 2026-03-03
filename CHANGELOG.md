# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- AI response rendered as formatted markdown (headings, bold, inline code, code blocks, lists, links)
- Clicking a link in the AI response opens it with `xdg-open`; copying still sends raw markdown to the clipboard
- Search bar magnifying glass replaced with a matching SVG icon (same stroke style as the robot icon)
- `/ai` inline AI assistant: type `/ai <question>` to query an AI provider without leaving the launcher
- Multi-provider support: OpenAI, Anthropic, Gemini, and Ollama (local)
- Scrollable response area with Copy and Retry icon buttons
- "Copied to clipboard" feedback on copy (auto-dismisses after 2 s via `wl-copy`)
- Animated loading indicator with trebuchet-themed verbs (Catapulting, Launching…)
- Robot icon in search bar while in AI mode; shake animation on empty prompt submit
- Escape in AI mode returns to the app grid (app stays open)
- `src/modes/` architecture: each mode owns its state and message handling, making future modes easy to add
  - `modes::search` — `SearchState` with filter, pagination, and selection logic
  - `modes::ai` — `AiStatus` + `AiState` with full AI lifecycle (query, retry, copy, tick)
- `ai_provider`, `ai_api_key`, `ai_model`, `ai_base_url` config keys with per-provider documentation
- Quoted config values are stripped automatically (e.g. `ai_provider = "anthropic"` works)
- `install.sh`: interactive AI setup wizard (provider menu, model, API key, base URL)
- `install.sh`: AI wizard is skipped when config is not freshly installed or overwritten
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
- `install.sh`: prompt before updating existing icons; skip with `--yes`
- `install.sh`: `confirm()` helper consolidates all interactive prompts

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
