# AGENTS.md

## Scope
- Applies to toolbar models, event bindings, and snapshot building under `src/ui/toolbar/`.
- Runtime Wayland/Cairo toolbar rendering is governed by `src/backend/wayland/toolbar/AGENTS.md`.

## Architecture
- `model/` defines toolbar state, control IDs, sliders, session/settings/tool models, and event policy; activation payloads are plain toolbar events.
- `snapshot/` and `snapshot.rs` build immutable snapshots consumed by runtime rendering and event handling.
- `bindings.rs` and `events.rs` connect toolbar models to application events without owning backend surface state.
- `session_format.rs` owns shared session-name normalization plus the built-in renderer's character-based truncation helpers.

## Invariants
- Keep this layer distinct from backend Cairo/Wayland toolbar layout, surfaces, and rendering.
- Snapshot data should be immutable after construction and should not perform durable state mutation.
- Toolbar events remain values; mutation belongs under `src/input/state/core/toolbar/apply/`.
- Keep model/event policy changes compatible with the unified top-toolbar runtime (layer-shell, inline, and GTK adapters).

## Coupled Changes
- Toolbar model, snapshot, and event changes often require updates to config toolbar settings, backend toolbar rendering/layout, action metadata, input state, docs, and tests.
- Control additions or renamed events may require configurator labels/search and command/help UI updates.

## Validation
- Add or update focused tests for toolbar snapshot building, event application, and input state mutations when behavior changes.
- Run targeted toolbar/input tests; use full local CI for broad toolbar behavior changes.
