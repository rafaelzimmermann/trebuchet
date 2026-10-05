---
version: alpha
name: Trebuchet
description: A keyboard-driven Wayland launcher with a translucent, fixed-size icon grid.
colors:
  background: "#14141eea"
  app_label: "#ffffff"
  app_selected: "#ffffff26"
typography:
  sans:
    fontFamily: "sans-serif"
rounded:
  panel: "16px"
  selected: "8px"
spacing:
  panel-vertical: "24px"
  panel-horizontal: "80px"
components:
  app_grid: {}
  search_bar: {}
---

## Overview

Trebuchet is a desktop utility for Hyprland/Wayland users who open applications
with a keybinding, fuzzy search, and keyboard navigation. Its signature is a
translucent overlay with large application icons, a quiet search bar, and a
stable grid. Existing English controls and desktop-provided app names are
preserved; no specific geographic market is established by the repository.

The runtime remains canonical: `src/theme.rs` defines default semantic colors,
`src/config.rs` loads user overrides, and shared widgets consume `Config.theme`.
This file records existing values, not a new generated theme. Architecture and
workflow context lives in `docs/architecture.md`.

## Colors

The dark translucent background keeps desktop context visible. White labels and
subtle selected-cell surfaces provide hierarchy. Custom themes retain the same
semantic roles. The documented colors map directly to the identically named
`Theme` fields, used by `src/app.rs` and `src/ui/grid.rs`.

## Typography

Use iced's default sans-serif font and system fallback. App labels use 13px text
in `src/ui/grid.rs`; preserve the existing search and panel typography.

## Layout

The layer-shell surface is 1000 × 860, centred on the active screen. Default
pagination is seven columns by five rows, with configurable dimensions and icon
size. `src/ui/mod.rs::PANEL_PADDING` owns panel padding. The shared grid pads short
rows and missing rows so filtering and icon loading preserve its footprint.

## Elevation & Depth

The overlay and translucent surface establish hierarchy. Do not add decorative
cards or extra shadows to individual application cells.

## Shapes

The outer panel radius is 16px (`src/app.rs`); selected cells use 8px
(`src/ui/grid.rs`). Fallback and resolved icons occupy the same dimensions.

## Components

`src/ui/grid.rs::app_grid` owns app/window grids. `src/ui/search.rs::search_bar`
owns search presentation. `AppLauncher` owns filtering, page, and keyboard
selection; `app.rs` coordinates background catalogue and icon loading.

Startup uses cached app metadata when available, then refreshes it from desktop
entries. Visible-page icons take priority; remaining icons resolve in bounded
background batches. Re-evaluate priority after navigation or search. Missing
icons keep the existing fallback without repeated resolution attempts. A refresh
preserves the current query and the selected application when it still exists;
removed selections clear and out-of-range pages clamp. Old icon batches cannot
write into a newer catalogue. Search remains local and immediate over the entire
loaded catalogue. Configuration changes update visible-page priority.

Keep the same keyboard and activation behavior across launcher and window mover.
This startup change introduces no new controls, animation, colors, or geometry.

## Do's and Don'ts

- Do preserve the current search and selection during background updates.
- Do keep launchable names available while icons load.
- Do use shared grid and theme owners for future presentation changes.
- Don't block visible icons on the complete offscreen icon set.
- Don't reset navigation when an unchanged catalogue finishes refreshing.
