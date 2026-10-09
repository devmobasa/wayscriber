# AGENTS.md

## Scope
- Applies to Wayland zoom capture, portal fallback, zoom state, and view transform code.

## Architecture
- `capture.rs` coordinates zoom capture.
- `portal.rs` provides portal fallback behavior.
- `state.rs` owns zoom image/view state and distinguishes direct and portal capture backends. `state/source.rs` owns identified request IDs, waiters, terminal reports, and the shared terminal cleanup that the lifecycle entry points in `state.rs` (deactivate, cancel, fail, stale direct capture) delegate to; `abort_capture` stays in `state.rs` because it ends a request without touching view or input state.
- `retry.rs` retains capture ID, waiter, requested activation, and selected backend through the shared layout settling policy and fresh preflight.
- `state.rs::handle_output_change` retains unbound capture requests during output discovery and the retry wait; bound requests and installed sources deactivate on an output change. `../state/core/output.rs` publishes output changes for protocol handlers and explicit switches; repeated enters preserve the current source.
- `view.rs` owns zoom view transforms.

## Invariants
- Preserve coordinate transforms, capture-source fallback behavior, and state transitions.
- Treat user cancellation as cancellation, not a hard failure.
- Portal crop and resampling run in the worker through `../portal_raster.rs`; full desktop changes invalidate portal admission without invalidating installed active-output sources.
- Keep zoom behavior aligned with frozen capture, render state, and input zoom actions.

## Coupled Changes
- Zoom changes may affect `src/input/state/actions/action_capture_zoom.rs`, `src/backend/wayland/state/zoom.rs`, `src/backend/wayland/state/render/`, and `src/capture/`.
- Portal behavior must stay coherent with `portal`/`dbus` feature gates.

## Validation
- Add focused tests around transform/state helpers where possible.
- Run targeted input/backend/capture tests for zoom behavior changes.
