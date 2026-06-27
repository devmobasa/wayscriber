# AGENTS.md

## Scope
- Applies to frozen-screen snapshot capture, image handling, portal fallback, and frozen state machine code.

## Architecture
- `capture.rs` coordinates frozen capture setup.
- `image.rs` owns frozen image data handling.
- `portal.rs` provides portal-based fallback behavior.
- `state.rs` tracks frozen-mode state.

## Invariants
- Preserve overlay-hide, capture, restore, and render ordering.
- Treat user cancellation as cancellation, not a hard failure.
- Keep coordinate and scale handling aligned with `src/backend/wayland/frozen_geometry.rs`, zoom behavior, and render state.

## Coupled Changes
- Frozen capture changes may affect `src/capture/`, `src/backend/wayland/zoom/`, and `src/backend/wayland/state/render/`.
- Portal changes must stay aligned with `portal`/`dbus` feature behavior.

## Validation
- Add focused tests around state or geometry helpers when possible.
- Run full local CI for broad capture or feature-gate changes.
