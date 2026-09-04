# AGENTS.md

## Scope
- Applies to drawing data, shapes, frames, canvas sets, dirty tracking, colors, fonts, and Cairo/Pango rendering helpers under `src/draw/`.

## Architecture
- `canvas_set/` owns multi-board/page canvas state.
- `frame/` owns frame storage, serialization, and undo/redo history.
- `shape/` owns shape types, bounds, text cache, polygons, step markers, and labels.
- `render/` owns Cairo/Pango rendering helpers.
- `render/context.rs` exposes `RenderCaches` for image/blur resources and the short-lived `RenderCtx` drawing borrow. Repeated rendering uses an explicit cache owner; public standalone drawing wrappers create local resources. Text measurement keeps its separate ownership and migration boundary.
- `spotlight.rs` owns shared Spotlight magnification defaults, normalization, formatting, and Serde boundaries.

## Invariants
- Keep this area mostly pure; rendering helpers should not mutate application state except intentional caches or Cairo surface/path operations.
- Preserve serialization compatibility, undo/history invariants, canvas/page identity, and Cairo path isolation.
- Keep drawing resources out of shapes, frames, snapshots, and input state. Preserve image payload identity, cache budgets, blur source/style keys, and uncached export blur behavior.

## Coupled Changes
- Drawing changes may affect input tools, selection behavior, canvas export, session snapshots, toolbar controls, config defaults, and tests.

## Validation
- Add focused tests for frame, shape, history, or canvas behavior.
- Use rendering/path-leakage tests for Cairo regressions.
- Run full local CI for serialized data or history changes.
