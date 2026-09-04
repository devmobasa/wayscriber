# AGENTS.md

## Scope
- Applies only to child modules under `state/`.
- The sibling root `src/backend/wayland/state.rs` is governed by `src/backend/wayland/AGENTS.md`.

## Architecture
- This subtree supports live overlay runtime state: buffers, damage, boards, capture routing, clipboard paste, color picker, onboarding, PDF export, render helpers, toolbar plumbing, zoom, and core accessors.
- Runtime owners extracted from `WaylandState` live beside it: `focus.rs` (activation, focus, and startup acquisition), `protocol_globals.rs` (bound globals and toolkit handler state), `pointer_runtime.rs` (pointer position, board-pan and chrome gestures, cursor, pointer-lock, and touch lifecycles), `region_capture/runtime.rs` (region generations, active/review/window-snap state, and query/preview workers), `acquisition.rs` (screen acquisition, zoom waiters, and eyedropper source correlation), `input_hud.rs` (system-reader lifecycle and reconciliation), `spotlight_runtime.rs` (render memory, warning latches, and wheel timing), `clipboard_runtime.rs` (single-flight workers and queue policy), `preference_stores.rs` (persistence stores and workers), `ui_animation.rs` (animation scheduling), `font_catalog.rs` (font-catalog prewarm), `text_input.rs` (text-input-v3 lifecycle and commit serials), `tablet_runtime.rs` (tablet-input-v2 objects and stylus contact), `key_repeat.rs` (manual key-repeat timing), and `helper_launch.rs` (About/configurator launches requested by input).
- `core/overlay.rs` owns suppression policy and capture-barrier state; `../surface.rs` owns output/fullscreen/layer placement and frozen-fullscreen transitions.
- `render/` owns overlay render phases; `toolbar/` owns `ToolbarChrome` (placement, inline interaction, and fade state), `ToolbarDrag` (built-in and GTK drag lifecycles), and runtime toolbar effects; `clipboard/` owns session paste helpers.

## Invariants
- Preserve snapshot boundaries for export and session actions.
- Keep render order, damage assumptions, buffer lifecycle, output identity, and toolbar visibility behavior explicit.
- State helpers may coordinate subsystems, but durable config, drawing, capture, session, and input rules should remain with their owning modules.

## Coupled Changes
- Render changes may affect `src/draw/`, `src/ui/`, `src/input/state/render.rs`, backend toolbar rendering, and visual tests.
- Toolbar state changes may affect `src/backend/wayland/toolbar/` and `src/ui/toolbar/`.
- Clipboard paste changes may affect `src/backend/wayland/clipboard/`, `src/file_uri.rs`, and input selection behavior.

## Validation
- Add focused tests near state helper modules when available.
- Run targeted backend/state/session/toolbar tests for runtime state changes.
