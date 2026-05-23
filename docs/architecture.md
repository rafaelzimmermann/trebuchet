# Trebuchet — Architecture & Design

## Overview

Trebuchet is a Wayland application launcher built for the [Hyprland](https://hyprland.org/) compositor. It appears as a translucent overlay centred on the active screen and provides:

- **App launcher** — real-time fuzzy search over all installed `.desktop` entries with an icon grid, keyboard navigation, and pagination.
- **AI assistant** (`/ai`) — query OpenAI, Anthropic, Gemini, or a local Ollama model; responses are rendered as formatted markdown.
- **Window mover** (`/mv`) — pick a window from another workspace and move it to the current one via `hyprctl`.
- **Command runner** (`/cmd`) — execute user-defined shell shortcuts, optionally capturing and displaying output.
- **Settings panel** (`/config`) — switch colour themes at runtime.

The application is written in Rust, uses [iced](https://github.com/iced-rs/iced) (0.14) with [iced-layershell](https://github.com/waycrate/exwlseat) for the Wayland layer-shell surface, and runs as a single-shot process that exits when it closes.

---

## High-Level Architecture

```
main.rs                      entry point — configures the layer-shell window and starts the iced runtime
 │
 └─ app.rs                   top-level state machine (Trebuchet struct)
     │                         owns Config, app list, and all five component instances
     │                         routes Message variants to the active component
     │
     ├─ components/           one self-contained module per UI mode
     │   ├─ component.rs      Component trait (handle_event / update / view / subscription)
     │   ├─ command.rs        SlashCommand parser, ComponentEvent enum
     │   ├─ app_launcher.rs   AppLauncher — app grid with fuzzy search
     │   ├─ ai_agent.rs       AIAgent — AI query lifecycle
     │   ├─ ai_client.rs      Async HTTP clients for OpenAI / Anthropic / Gemini / Ollama
     │   ├─ window_mover.rs   WindowMover — hyprctl JSON + move dispatch
     │   ├─ cmd.rs            Cmd — shell command runner
     │   └─ settings.rs       Settings — theme switching
     │
     ├─ ui/                   shared UI building blocks
     │   ├─ mod.rs            PANEL_PADDING constant
     │   ├─ search.rs         search_bar widget + ShakeState animation
     │   ├─ grid.rs           app_grid widget (generic over message type)
     │   ├─ panel.rs          icon_btn, COPY_ICON, PanelState enum
     │   └─ ai_response.rs    ai_panel widget with markdown rendering + model picker
     │
     ├─ launcher.rs           scan_applications(), launch_app(), clean_exec()
     ├─ icons.rs              icon resolution pipeline (embedded → manifest → system)
     ├─ config.rs             INI config parser with layered defaults
     └─ theme.rs              Theme struct with 22 colour keys loaded from .conf files
```

---

## Core Architectural Decisions

### 1. Single-shot process (exit-on-close)

Trebuchet is launched by a keybinding, opens instantly, and calls `std::process::exit(0)` when it closes (cursor leaves the window, Escape is pressed in launcher mode, or an app is launched). There is no daemon mode or background service.

**Rationale:** Launcher UX demands zero latency between keypress and visible window. Starting a fresh process per invocation is simpler, avoids stale state, and the binary starts fast enough that the overhead is imperceptible.

### 2. Component trait pattern

Every UI mode implements a shared `Component` trait:

```rust
trait Component {
    type Msg: Clone + Debug + Send + 'static;
    fn handle_event(&mut self, event: &Event, status: Status, apps: &[AppEntry], config: &Config)
        -> (Task<Self::Msg>, ComponentEvent);
    fn update(&mut self, msg: Self::Msg, apps: &[AppEntry], config: &Config)
        -> (Task<Self::Msg>, ComponentEvent);
    fn view<'a>(&'a self, apps: &'a [AppEntry], config: &'a Config) -> Element<'a, Self::Msg>;
    fn subscription(&self) -> Subscription<Self::Msg>;
}
```

`app.rs` is a pure message router — it holds all five component structs but only delegates to the active one. Cross-component navigation (e.g. typing `/ai` in the app launcher to switch to AI mode) is communicated via `ComponentEvent::CommandInvoked(SlashCommand, args)` returned from the active component and dispatched centrally in `apply_event()`.

**Rationale:** Prevents mode-specific logic from leaking into the top level, allows each mode to evolve independently, and provides a uniform interface for testing.

### 3. Dual input path (raw keyboard events + text_input widget)

Components receive keyboard input through two channels:
- **`handle_event`** receives raw `KeyPressed` events from the iced event subscription. This is used for navigation keys (arrows, Enter, Escape, PageUp/Down), character input, and backspace. The `Status::Ignored` guard ensures that if a widget (like the text_input) has already consumed the event, the component does not double-process it.
- **`update`** receives `QueryChanged` messages from the `text_input::on_input` callback, used when the search bar's iced widget forwards typed text.

In practice, most character/key handling goes through `handle_event` because the component directly manages the query string. The `text_input` widget is rendered but primarily provides the visual text field and cursor.

**Rationale:** iced's layer-shell integration does not automatically route text input to the focused widget the way a standard window would. Handling raw keyboard events gives full control over input processing and slash-command detection.

### 4. Slash commands as navigation triggers

Slash commands (`/ai`, `/mv`, `/cmd`, `/config`, `/app`) are detected in two ways:
- **Space-triggered:** as the user types, `SlashCommand::detect()` checks for the `/<command> ` pattern. This provides instant mode switching without needing Enter.
- **Enter-triggered:** on submit, the query is checked with `SlashCommand::as_nav_event()`.

Both paths produce a `ComponentEvent::CommandInvoked` that `app.rs` translates into an `ActiveComponent` transition.

**Rationale:** Space-triggered dispatch makes mode switching feel instantaneous — `/ai ` drops the user straight into AI mode as soon as the space is typed, with the `/ai ` prefix consumed automatically. The Enter path is a fallback for users who type the command and then press Enter.

### 5. Layered config with embedded defaults

Configuration is loaded in three layers, each overriding the previous:

1. **Hardcoded Rust defaults** (`Config::default()`)
2. **Embedded config** (`assets/trebuchet.conf` compiled into the binary via `include_str!`)
3. **User config** (`~/.config/trebuchet/trebuchet.conf`)

The parser is a simple line-by-line INI reader that also handles `[[command]]` and `[[ai_model]]` TOML-like array-of-tables blocks. Legacy flat AI keys (`ai_provider`, `ai_api_key`, `ai_model`) are supported for backward compatibility.

**Rationale:** Embedded defaults ensure the binary works out-of-the-box without any config file. The layered approach means users only need to specify the keys they want to change. The custom parser avoids adding a heavy TOML/INI dependency for a format that is trivially parseable.

### 6. Icon resolution pipeline

Icons are resolved through a multi-strategy pipeline (first match wins):

1. **Embedded SVG/PNG assets** (`assets/icons/` compiled in via `rust-embed`) — high-resolution fallbacks fetched by `scripts/fetch-icons.sh`.
2. **Manifest alias lookup** — `assets/icons/manifest.json` (also embedded) maps `wm_class`, `icon_name`, and `app_name` to embedded filenames, bridging the gap between what `.desktop` files call an icon and what the file is actually named.
3. **Absolute path** from the `Icon=` desktop entry key.
4. **System icon theme directories** — standard XDG paths (`hicolor`, `pixmaps`) at various resolutions.

For window mover icons specifically, `icon_for_window()` adds additional heuristics: lowercased class, last dot-segment of reverse-DNS classes (`com.mitchellh.ghostty` → `ghostty`), display-name lookup, and manifest `wm_class` matching.

**Rationale:** Desktop Linux icon naming is inconsistent. The multi-strategy pipeline handles the common cases (Papirus names, reverse-DNS bundle IDs, mismatched WM classes) without requiring the user to configure anything.

### 7. Async operations with Tokio

All potentially blocking work is offloaded from the iced render loop:

- **App scanning:** `scan_applications()` runs via `tokio::task::spawn_blocking` so the window appears immediately while `.desktop` files are parsed on a thread pool (parallelised with Rayon).
- **AI queries:** HTTP requests to providers run as `Task::perform` async futures.
- **Window fetching:** `hyprctl clients -j` and `hyprctl activeworkspace -j` run as async `tokio::process::Command`.
- **Shell commands:** `display_result` commands in `/cmd` run via `tokio::process::Command` with a "Running…" indicator.
- **Clipboard:** `wl-copy` is spawned as a subprocess (non-blocking).

**Rationale:** The iced runtime is single-threaded for UI updates. Any blocking I/O on the main thread would cause the UI to freeze. Tokio's multi-threaded runtime provides the necessary concurrency.

### 8. Theme system

Themes are plain `.conf` files with `key = #RRGGBB` or `key = #RRGGBBAA` colour definitions. The `Theme` struct holds 22 colour keys covering all UI elements. Themes are loaded at startup (from `~/.config/trebuchet/themes/<name>.conf`) and can be switched at runtime via `/config → theme <name>`, which persists the selection to `~/.config/trebuchet/current-theme`.

**Rationale:** Plain key=value files are easy to create and edit. The fixed set of 22 keys keeps the theme system simple while covering every visual element in the application.

---

## Key Data Flows

### Application launch

```
User types → handle_event (char input) → query updated → fuzzy filter re-run
User presses Enter → handle_submit → launch_app() → clean_exec() → std::process::Command → exit(0)
```

### AI query

```
User types /ai question → Enter → do_submit() → start_query() → Task::perform(ai_client::query)
                                                                       ↓
                                                              HTTP POST to provider
                                                                       ↓
                                                              Msg::Response(Ok/Err)
                                                                       ↓
                                                              AiStatus::Done/Error
                                                              markdown::parse → response_items
```

### Window move

```
User types /mv → SlashCommand detected → ComponentEvent::CommandInvoked(Mv, args)
                                         ↓
                                   app.rs switches to WindowMover
                                   WindowMover::reset() → Task::perform(fetch_windows)
                                                                 ↓
                                                         hyprctl clients -j / activeworkspace -j
                                                                 ↓
                                                         Msg::WindowsLoaded → windows Vec
                                                                 ↓
User selects window → dispatch_move() → Task::perform(move_window)
                                                  ↓
                                          hyprctl dispatch movetoworkspacesilent
                                                  ↓
                                          Msg::WindowMoved → ComponentEvent::Exit
```

---

## Wayland Integration

Trebuchet uses the `wlr-layer-shell` protocol via `iced-layershell`:

| Setting | Value | Purpose |
|---------|-------|---------|
| `layer` | `Overlay` | Renders above all other surfaces |
| `anchor` | `empty` | No edge anchoring — compositor centres the surface |
| `exclusive_zone` | `0` | Does not reserve screen space |
| `keyboard_interactivity` | `Exclusive` | Captures all keyboard input while visible |
| `start_mode` | `Active` | Opens on the currently active output |
| `size` | `(1000, 860)` | Fixed window size tuned for 7×5 grid at 96px icons |

The window background is fully transparent. The visible rounded container is drawn by the `view()` function in `app.rs`, allowing the compositor to clip corners cleanly.

**Closing behaviour:**
- Cursor leaving the window (`CursorLeft` event) closes the launcher.
- Clicks in the margin (outside the content area, `Status::Ignored`) close the launcher.
- Clicks inside the content area are absorbed by a `mouse_area` wrapper and do not close.

---

## Window Mover — Hyprland Dependency

The window mover is Hyprland-specific:

- **Discovery:** `hyprctl clients -j` returns all open windows as JSON; `hyprctl activeworkspace -j` identifies the current workspace.
- **Filtering:** Windows on the active workspace are excluded.
- **Sorting:** Windows are ordered by workspace ID, then by horizontal position.
- **Moving:** `hyprctl dispatch movetoworkspacesilent "<workspace>,address:<addr>"` silently pulls the window.
- **Exit:** After a successful move, the launcher exits (`ComponentEvent::Exit`).

---

## AI Provider Integration

Four providers are supported, each with a dedicated async function in `ai_client.rs`:

| Provider | Default model | Auth method | Endpoint |
|----------|--------------|-------------|----------|
| OpenAI | `gpt-4o` | Bearer token (`api_key`) | `api.openai.com/v1/chat/completions` |
| Anthropic | `claude-opus-4-6` | `x-api-key` header | `api.anthropic.com/v1/messages` |
| Gemini | `gemini-2.0-flash` | Query param `?key=` | `generativelanguage.googleapis.com/v1beta/models/…` |
| Ollama | `llama3.2` | None | `localhost:11434/api/chat` |

All providers use `reqwest` with `rustls-tls` (no native TLS dependency). The `base_url` override enables compatible proxies or alternative endpoints.

---

## Testing Strategy

Tests are co-located in each module under `#[cfg(test)] mod tests`. Key test areas:

| Module | What is tested |
|--------|---------------|
| `config.rs` | Config parsing (defaults, overrides, `[[command]]` and `[[ai_model]]` blocks, legacy keys, accumulation across layers) |
| `launcher.rs` | `clean_exec()` field-code stripping |
| `icons.rs` | `name_candidates()` slug generation |
| `app.rs` | Event routing (cursor-left closes, margin click closes, captured click does not close) |
| `command.rs` | `SlashCommand::detect()` and `as_nav_event()` parsing |
| `app_launcher.rs` | Filter, pagination, page navigation |
| `ai_agent.rs` | Copy/retry when idle/loading returns `Handled` (regression) |
| `cmd.rs` | Execute lifecycle, async output handling, copy safety |
| `settings.rs` | Copy when idle safety, reset behaviour |

Run all tests:

```sh
cargo test
```

---

## External Dependencies

| Dependency | Version | Purpose |
|-----------|---------|---------|
| `iced` | 0.14 | UI framework ( Elm-inspired, declarative) |
| `iced_layershell` | 0.15 | Wayland layer-shell backend for iced |
| `freedesktop-desktop-entry` | 0.8 | Parse `.desktop` files |
| `xdg` | 3.0 | XDG base-directory resolution |
| `fuzzy-matcher` | 0.3 | Skim-based fuzzy search |
| `tokio` | 1 | Async runtime (multi-thread, process, time) |
| `rayon` | 1 | Data-parallel app scanning |
| `reqwest` | 0.12 | HTTP client (rustls TLS) |
| `serde` / `serde_json` | 1 | JSON parsing (hyprctl output, manifest) |
| `rust-embed` | 8 | Compile-time embedding of `assets/icons/` |

### Runtime requirements

- **Wayland compositor** supporting `wlr-layer-shell` (Hyprland, Sway, etc.)
- **`hyprctl`** — required for the window mover; other features work without it
- **`wl-copy`** — optional, for clipboard support
- **`xdg-open`** — optional, for opening links from AI responses
- **Rust toolchain** — stable, 2021 edition or later

---

## File Layout

```
trebuchet/
├── Cargo.toml
├── assets/
│   ├── trebuchet.conf          # embedded default config
│   └── icons/                  # fetched SVG/PNG icons + manifest.json
├── scripts/
│   ├── fetch-icons.sh          # download high-res icons, generate manifest.json
│   └── install.sh              # build + system-wide install
├── src/
│   ├── main.rs                 # entry point, layer-shell configuration
│   ├── app.rs                  # top-level state machine, message router
│   ├── config.rs               # layered INI config parser
│   ├── theme.rs                # 22-key colour theme system
│   ├── icons.rs                # icon resolution pipeline
│   ├── launcher.rs             # .desktop scanner, app launcher logic
│   ├── components/
│   │   ├── component.rs        # Component trait
│   │   ├── command.rs          # SlashCommand parser, ComponentEvent enum
│   │   ├── app_launcher.rs     # app grid mode
│   │   ├── ai_agent.rs         # AI assistant mode
│   │   ├── ai_client.rs        # HTTP clients for 4 AI providers
│   │   ├── window_mover.rs     # window mover mode
│   │   ├── cmd.rs              # shell command runner mode
│   │   └── settings.rs         # theme settings mode
│   └── ui/
│       ├── mod.rs              # shared constants, re-exports
│       ├── search.rs           # search bar widget, shake animation
│       ├── grid.rs             # app icon grid widget
│       ├── panel.rs            # icon button, PanelState enum
│       └── ai_response.rs      # AI response panel with markdown rendering
└── docs/
    └── architecture.md         # this file
```

---

## Configuration Reference

Config file: `~/.config/trebuchet/trebuchet.conf`

| Key | Default | Description |
|-----|---------|-------------|
| `columns` | `7` | Grid columns per page |
| `rows` | `5` | Grid rows per page |
| `icon_size` | `96` | Icon size in pixels |

### `[[ai_model]]` block (repeatable)

| Key | Required | Description |
|-----|----------|-------------|
| `provider` | Yes | `openai`, `anthropic`, `gemini`, or `ollama` |
| `api_key` | For cloud providers | API key |
| `model` | No | Comma-separated model IDs |
| `base_url` | No | Override API endpoint |

### `[[command]]` block (repeatable)

| Key | Required | Description |
|-----|----------|-------------|
| `prefix` | Yes | Trigger prefix (e.g. `shutdown`) |
| `command` | Yes | Shell command to run |
| `display_result` | No | `true` to capture and display stdout (default: `false`) |

---

## Version History Summary

| Version | Date | Highlights |
|---------|------|-----------|
| 0.0.1 | 2026-03-02 | Initial scaffold — grid layout, search, icons, install script |
| 0.1.0 | 2026-03-03 | AI assistant, multi-provider support, markdown rendering |
| 0.2.0 | 2026-03-04 | Custom commands, component isolation refactor |
| 0.3.0 | 2026-03-07 | `/cmd` runner, `/config` panel, model picker, `[[ai_model]]` blocks |
| 0.3.1 | 2026-03-07 | Click-handling fixes (margin vs content), shared PANEL_PADDING |
| 0.4.0 | 2026-03-08 | Window mover, icon manifest, shared icons module |
| 0.4.1 | 2026-03-22 | Performance: spawn_blocking + Rayon for app scanning |
