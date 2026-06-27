# AGENTS.md

## Scope
- Applies to backend abstraction code under `src/backend/`.

## Architecture
- `mod.rs` exposes the backend entry surface.
- `wayland/` owns the concrete Wayland backend implementation.

## Invariants
- Keep the public backend surface small; active overlay mode should enter through the backend API rather than reaching into Wayland internals.
- Backend code translates compositor/protocol events into input, capture, session, and render operations.
- Avoid blocking runtime/event-loop paths.

## Coupled Changes
- Backend API changes may affect `src/app/`, `src/daemon/`, tests, and docs.

## Validation
- Run backend-focused checks and clippy for runtime changes.
- Run full local CI for public backend API changes.
