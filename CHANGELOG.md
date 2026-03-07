# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added
- `/cmd` command runner: type `/cmd` (+ space or Enter) to open a dedicated terminal-style panel for running configured shell commands; type a prefix and press Enter to trigger it; ESC returns to the launcher
- `/config` panel now lists available themes at idle — no sub-command needed to see them
- Model picker at the bottom-left of the AI panel: switch between configured models mid-session without touching the config
- `[[ai_model]]` config block with comma-separated `model` field — one block per provider, one entry per model in the picker (e.g. `model = claude-sonnet-4-6, claude-opus-4-6`)
- Picker labels are auto-generated as `provider:model` (e.g. `anthropic:claude-sonnet-4-6`)
- Multiple `[[ai_model]]` blocks supported; the first model of the first block is the default

### Changed
- Custom command prefixes in `trebuchet.conf` no longer need a leading `/`; existing configs with `/prefix` continue to work (the slash is silently stripped on load)
- Navigation slash commands (`/ai`, `/app`, `/config`, `/cmd`) can be triggered with Space or Enter from within any panel, not just the launcher
- Unknown slash commands typed in any panel now shake the search bar instead of silently clearing
- AI config restructured: four flat keys (`ai_provider`, `ai_api_key`, `ai_model`, `ai_base_url`) replaced by `[[ai_model]]` blocks; flat keys still work as a single-model shorthand for backward compatibility
- `install.sh`: all interactive prompts (icons update, config overwrite, AI setup wizard) are now gathered upfront before the build starts, so the installation runs unattended once questions are answered
- `fetch-icons.sh`: added `FETCH_AS` alias map so icon save-name can differ from the Papirus/Simple-Icons lookup name; removed unavailable icons (cohere, xai, orca-slicer, pamac-manager)
- Search bar placeholder text is now mode-aware: "Search apps..." in launcher mode, "Ask anything..." in AI mode, empty in terminal/command mode

### Fixed
- `/cmd` commands with `display_result = true` no longer freeze the launcher while running; the panel immediately shows a "Running…" indicator and remains fully interactive until the result arrives

### Performance
- `/config` panel opens faster; the installed theme list is now read from disk once when the panel opens instead of on every render frame

### Removed
- Direct slash-command execution from the launcher search bar; custom commands are now accessed through the `/cmd` panel

## [0.2.0] - 2026-03-04

### Added
- Custom commands via `[[command]]` blocks in `trebuchet.conf`: type a prefix and press Enter (or space) to run a shell command; set `display_result = true` to capture stdout and display it in a dedicated terminal-style panel
- Dedicated command-result view: terminal icon in the search bar, monospace output with green prompt line, copy button, input cleared after each run so the next command can be typed immediately
- `/ai ` (trailing space) now immediately switches to AI mode without requiring Enter; same for custom command prefixes
- `install.sh`: `--no` / `-n` flag — assumes "no" to all interactive prompts (opposite of `--yes`)

### Changed
- Component isolation completed: all mode logic moved to `src/components/`; `app.rs` is a pure router with no mode-specific knowledge
- `Component::update()` now returns `(Task<Msg>, ComponentEvent)` so widget-driven input (e.g. `text_input` on_input) can trigger cross-component transitions without going through `handle_event`
- Each mode's search bar uses a distinct widget ID; switching modes no longer carries over text from the previous input field
- `Component` trait simplified: `handle_event` replaces seven individual handler methods

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
