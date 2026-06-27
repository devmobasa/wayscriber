# AGENTS.md

## Scope
- Applies to Wayland zoom capture, portal fallback, zoom state, and view transform code.

## Architecture
- `capture.rs` coordinates zoom capture.
- `portal.rs` provides portal fallback behavior.
- `state.rs` owns zoom runtime state.
- `view.rs` owns zoom view transforms.

## Invariants
- Preserve coordinate transforms, capture-source fallback behavior, and state transitions.
- Treat user cancellation as cancellation, not a hard failure.
- Keep zoom behavior aligned with frozen capture, render state, and input zoom actions.

## Coupled Changes
- Zoom changes may affect `src/input/state/actions/action_capture_zoom.rs`, `src/backend/wayland/state/zoom.rs`, `src/backend/wayland/state/render/`, and `src/capture/`.
- Portal behavior must stay coherent with `portal`/`dbus` feature gates.

## Validation
- Add focused tests around transform/state helpers where possible.
- Run targeted input/backend/capture tests for zoom behavior changes.
