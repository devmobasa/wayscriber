# AGENTS.md

## Scope
- Applies to Wayland backend startup, setup, event-loop phases, state initialization, signal handling, tray wiring, rendering dispatch, capture polling, and session save on exit.

## Architecture
- `run.rs` is the active backend run entry.
- `setup.rs`, `surface.rs`, and `state_init/` build the initial Wayland state.
- `event_loop/` owns dispatch, render, capture polling, and exit-time session save phases.
- `signals.rs` and `tray.rs` integrate process lifecycle and daemon/tray behavior.

## Invariants
- Keep event-loop phases explicit and non-blocking.
- Do not perform slow file, process, clipboard, or capture work inline in dispatch/render paths.
- Preserve signal behavior, tray feature gating, frame callback assumptions, and session-save ordering.

## Coupled Changes
- Startup and state-init changes may affect `src/backend/wayland/state.rs`, `src/input/`, `src/config/`, `src/session/`, and daemon mode.
- Tray changes must stay aligned with `tray` feature behavior, daemon lifecycle, configurator daemon setup, and packaging service files.

## Validation
- Add focused tests near changed event-loop/session-save helpers when available.
- Run clippy for backend lifecycle changes.
- Run full local CI for broad startup, feature-gate, or event-loop changes.
