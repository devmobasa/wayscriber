# AGENTS.md

## Scope
- Applies to drawing data, shapes, frames, canvas sets, dirty tracking, colors, fonts, and Cairo/Pango rendering helpers under `src/draw/`.

## Architecture
- `canvas_set/` owns multi-board/page canvas state.
- `frame/` owns frame storage, serialization, and undo/redo history.
- `shape/` owns shape types, bounds, text cache, polygons, step markers, and labels.
- `render/` owns Cairo/Pango rendering helpers.

## Invariants
- Keep this area mostly pure; rendering helpers should not mutate application state except intentional caches or Cairo surface/path operations.
- Preserve serialization compatibility, undo/history invariants, canvas/page identity, and Cairo path isolation.

## Coupled Changes
- Drawing changes may affect input tools, selection behavior, canvas export, session snapshots, toolbar controls, config defaults, and tests.

## Validation
- Add focused tests for frame, shape, history, or canvas behavior.
- Use rendering/path-leakage tests for Cairo regressions.
- Run full local CI for serialized data or history changes.
