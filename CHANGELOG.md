# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- Bare slash command + Enter switches mode: `/ai` ↵ enters AI mode, `/app` ↵ returns to launcher (previously a trailing space was required)

### Changed
- `Component` trait reduced to a single `handle_event(event, status, apps, config)` method; components parse raw iced events directly instead of receiving pre-translated message variants
- `app::Message` reduced from 14 variants to 4 (`Close`, `IcedEvent`, `Launcher`, `Ai`); `on_event` is now a trivial pass-through
- Removed `NavDirection` enum and all `dispatch_input`/`dispatch_nav` helpers from `app.rs`

## [0.1.0] - 2026-03-03

### Added
- AI response rendered as formatted markdown (headings, bold, inline code, code blocks, lists, links)
- Clicking a link in the AI response opens it with `xdg-open`; copying sends raw markdown to clipboard
- Search bar magnifying glass replaced with a matching SVG icon (same stroke style as the robot icon)
- `/ai` inline AI assistant: type `/ai <question>` to query an AI provider without leaving the launcher
- Multi-provider support: OpenAI, Anthropic, Gemini, and Ollama (local)
- Scrollable response area with Copy and Retry icon buttons
- "Copied to clipboard" feedback on copy (auto-dismisses after 2 s via `wl-copy`)
- Animated loading indicator with trebuchet-themed verbs (Catapulting, Launching…)
- Robot icon in search bar while in AI mode; shake animation on empty prompt submit
- Escape in AI mode returns to the app grid; type `/app ` to do the same
- `ai_provider`, `ai_api_key`, `ai_model`, `ai_base_url` config keys with per-provider documentation
- Quoted config values are stripped automatically (e.g. `ai_provider = "anthropic"` works)
- `install.sh`: interactive AI setup wizard (provider menu, model, API key, base URL)
- `install.sh`: AI wizard is skipped when config is not freshly installed or overwritten
- `Terminal=true` desktop entry support: apps requiring a terminal are launched inside an auto-detected emulator (`$TERMINAL`, foot, kitty, alacritty, ghostty, wezterm, xterm)
- `AppEntry::terminal` field parsed from desktop entry files
- File-based configuration: `~/.config/trebuchet/trebuchet.conf` loaded at startup
- `assets/trebuchet.conf` embedded in the binary as the authoritative default config
- Config loading is layered: hardcoded Rust defaults → embedded conf → user conf
- Unit tests for config parsing (missing keys, invalid values, unknown keys, whitespace, comments)
- Updated README: configuration section, keyboard navigation, terminal app support, AI assistant
- `install.sh`: deploy default config on install; prompt before overwriting an existing one
- `install.sh`: `--uninstall` removes `~/.config/trebuchet/` in addition to the binary and icons
- `install.sh`: `--yes` / `-y` flag for non-interactive installs
- `install.sh`: `confirm()` helper consolidates all interactive prompts

### Changed
- Component isolation refactor: `app.rs` is now a pure message router; each UI mode is a self-contained component implementing a `Component` trait
  - `app_launcher.rs` — `AppLauncher` owns query, filter, pagination, selection, and shake state
  - `ai_agent.rs` — `AIAgent` owns query, prompt, AI status, copy feedback, and shake state
  - `command.rs` — `SlashCommand` + `ComponentEvent` for cross-component communication
  - `component.rs` — `Component` trait and `NavDirection` enum
  - `ui/search.rs` — `ShakeState` moved here; `search_bar` is now generic over message type
  - `ui/grid.rs` — `app_grid` is now generic over message type via an `on_activate` callback
  - `ui/ai_response.rs` — `ai_panel` is now generic over message type via callbacks
- AI mode search bar no longer shows the `/ai ` prefix; only the question text is displayed

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
