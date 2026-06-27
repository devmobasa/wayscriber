# AGENTS.md

## Scope
- Applies to input-side toolbar model, event bindings, snapshot building, and apply logic under `src/ui/toolbar/`.
- Runtime Wayland/Cairo toolbar rendering is governed by `src/backend/wayland/toolbar/AGENTS.md`.

## Architecture
- `model/` defines toolbar state, controls, activation/session/settings/tool models, and event policy.
- `snapshot/` and `snapshot.rs` build immutable snapshots consumed by runtime rendering and event handling.
- `apply/` mutates `InputState` from toolbar events, page changes, board changes, action requests, delay/layout changes, and tool selections.
- `bindings.rs` and `events.rs` connect toolbar models to application events without owning backend surface state.

## Invariants
- Keep this layer distinct from backend Cairo/Wayland toolbar layout, surfaces, and rendering.
- Snapshot data should be immutable after construction and should not perform durable state mutation.
- Apply code should route mutations through `InputState` and preserve existing action/keybinding semantics.
- Keep model/event policy changes compatible with top-strip and side-palette runtime behavior.

## Coupled Changes
- Toolbar model/snapshot/apply changes often require updates to config toolbar settings, backend toolbar rendering/layout, action metadata, input state, docs, and tests.
- Control additions or renamed events may require configurator labels/search and command/help UI updates.

## Validation
- Add or update focused tests for toolbar snapshot building, event application, and input state mutations when behavior changes.
- Run targeted toolbar/input tests; use full local CI for broad toolbar behavior changes.
