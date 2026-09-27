# Rust correctness and maintenance plan

Implement the seven confirmed review findings before broader architectural work.
Keep the existing launcher layout and navigation, and preserve Hyprland Lua and
legacy support. Each fix needs a regression test that demonstrates the defect.

## TODO — implementation order

- [x] Validate configuration dimensions: columns/rows 1–32, icon size 1–512;
      reject invalid values while retaining the preceding configuration value.
      (`src/config.rs` — range-checked parses; regression test
      `invalid_dimensions_keep_previous_values`.)
- [x] Reset command-block fields even when a block is incomplete; preserve shell
      quoting and only remove a matching outer configuration quote pair.
      (`src/config.rs` — `finalize_cmd` always drains block state; quote-aware
      value unwrapping; tests `incomplete_blocks_do_not_leak_fields`,
      `preserves_shell_quotes_and_supports_outer_quotes`.)
- [x] Make slash-command whitespace and color parsing safe for all UTF-8 input;
      accept only six/eight ASCII hexadecimal digits for colors.
      (`src/components/command.rs` — whitespace handled by
      `trim_start_matches(char::is_whitespace)` on a char boundary;
      `src/theme.rs` — `parse_color` requires exactly 6/8 ASCII hex digits;
      tests `unicode_whitespace_preserves_argument_boundaries`,
      `malformed_colors_never_panic`.)
- [x] Replace whitespace-based desktop Exec parsing with specification-aware
      argument parsing and field expansion; launch terminal apps with explicit
      arguments, and propagate launch failures. (New `src/desktop_exec.rs`
      covering desktop-entry escape decoding, Exec quoting rules and field-code
      expansion/removal with rejection of misplaced codes;
      `src/launcher.rs::launch_app` takes `&[String]` and returns
      `Result<(), String>`; terminal wrapping uses explicit
      `<emulator> <flag> program args`; launch failures shake and keep the
      launcher open.)
- [x] Preserve custom-command exit status, stdout and stderr and report
      failures. (New `src/process.rs` — `CommandOutput` with status + both
      streams and a `display()` summary including
      `Command failed (<status>)`; spawn failure of silent commands is shown
      in the result panel instead of being dropped.)
- [x] Associate async command/window results with request IDs; invalidate
      results after navigation/reset and prevent duplicate in-flight move
      dispatches. (`Cmd`/`WindowMover` carry `u64` request IDs bumped by
      `leave()`/`reset()` and from `apply_event` on every navigation; late
      messages are dropped; `moving` flag guards concurrent `hyprctl dispatch`.)
- [x] Add regression tests, fix existing Clippy findings, run formatting,
      tests, strict Clippy, release build and available live Hyprland checks.
      (143 unit tests + 2 ignored live tests pass; Clippy clean with
      `-D warnings` after fixing `items_after_test_module` and
      `unnecessary_get_then_check`; `cargo fmt` applied.)
- [x] Update configuration/architecture documentation and record verification.
      (`docs/architecture.md`, `README.md`, `CHANGELOG.md` updated; this
      verification record.)

## Follow-up backlog (separate changes)

- Graceful application shutdown and explicit ownership of child processes.
- Consistent XDG configuration/cache/data lookup and desktop-entry precedence.
- Move remaining filesystem work off event handlers; atomic cache/theme writes.
- Shared pagination state and clamping when asynchronously loaded settings change.

## Verification record

Verified 2026-09-27 on Hyprland 0.56.2 (live session, legacy dispatch rejected →
Lua fallback exercised):

| Check | Command | Result |
|-------|---------|--------|
| Unit + regression tests | `cargo test` | 143 passed, 2 ignored |
| Live Hyprland checks | `cargo test --release -- --ignored` | `live_hyprland_window_query`, `live_lua_dispatch_retry` both ok |
| Strict Clippy | `cargo clippy --all-targets -- -D warnings` | clean (fixed `items_after_test_module` in `app.rs`, `unnecessary_get_then_check` in `icons.rs`) |
| Formatting | `cargo fmt --check` | clean |
| Release build | `cargo build --release` | ok |

Regression coverage added for the seven findings:

- `config::tests::invalid_dimensions_keep_previous_values`
- `config::tests::incomplete_blocks_do_not_leak_fields`
- `config::tests::preserves_shell_quotes_and_supports_outer_quotes`
- `command::tests::unicode_whitespace_preserves_argument_boundaries`
- `theme::tests::malformed_colors_never_panic`
- `desktop_exec::tests::*` (quoting, escapes, field codes, rejections)
- `process::tests::preserves_failure_status_and_both_streams`,
  `process::tests::successful_empty_output_and_stderr_are_distinct`
- `cmd`/`window_mover` request-ID staleness and duplicate-dispatch guards

Out of scope for this pass (tracked in the backlog above): graceful shutdown
with explicit child ownership, XDG lookup consistency, off-thread filesystem
work, and shared pagination state.
