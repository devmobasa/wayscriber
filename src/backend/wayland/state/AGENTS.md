# AGENTS.md

## Scope
- Applies only to child modules under `state/`.
- The sibling root `src/backend/wayland/state.rs` is governed by `src/backend/wayland/AGENTS.md`.

## Architecture
- This subtree supports live overlay runtime state: buffers, damage, boards, capture routing, clipboard paste, color picker, onboarding, PDF export, render helpers, toolbar plumbing, zoom, and core accessors.
- `render/` owns overlay render phases; `toolbar/` owns runtime toolbar state helpers; `clipboard/` owns session paste helpers.

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
