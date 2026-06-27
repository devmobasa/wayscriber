# AGENTS.md

## Scope
- Applies to runtime toolbar layout, hit testing, event types, rows, surfaces, rendering, widgets, and main toolbar state.

## Architecture
- `layout/` computes top-strip and side-palette geometry.
- `render.rs` and `render/` draw toolbar surfaces and widgets.
- `surfaces/` owns Wayland surface lifecycle, buffers, hit testing, and surface rendering.
- `main/` owns runtime toolbar state and lifecycle.
- `events.rs`, `hit.rs`, and `rows.rs` connect runtime events, hit targets, and row definitions.

## Invariants
- Keep layout geometry, hit regions, event snapshots, and Cairo rendering aligned.
- Preserve top-strip versus side-palette behavior, drawer/collapsible sections, session rows, delay sliders, preset rows, tool rows, and event policy.
- Child guides under split-module directories must not govern sibling roots such as `render.rs`.

## Coupled Changes
- Runtime toolbar changes may affect `src/backend/wayland/state/toolbar.rs`, `src/backend/wayland/state/toolbar/`, `src/ui/toolbar/`, `src/input/`, toolbar icons, config toolbar settings, and action metadata.

## Validation
- Add or update layout tests under `layout/tests/` for geometry changes.
- Use rendering smoke tests or targeted tests for render/hit/surface changes.
- Run full local CI for broad toolbar behavior changes.
