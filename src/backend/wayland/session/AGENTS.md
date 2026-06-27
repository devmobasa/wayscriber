# AGENTS.md

## Scope
- Applies only to helper modules under `session/`.
- The sibling root `src/backend/wayland/session.rs` is governed by `src/backend/wayland/AGENTS.md`.

## Architecture
- This subtree supports runtime session operations for the live overlay.
- It should work with core session validation, snapshots, catalog metadata, and input session preflight instead of duplicating those rules.

## Invariants
- Preserve save-before-open behavior, target validation, user prompt boundaries, rollback behavior, and catalog updates.
- Do not replace active board state until a runtime open has validated and loaded successfully.
- Keep named-session safety aligned with `src/session/` and `src/paths/`.

## Coupled Changes
- Runtime session changes may affect `src/session/`, `src/input/state/core/session_preflight*`, daemon named-session switching, and configurator session catalog code.

## Validation
- Add focused tests under this subtree or `src/session/` for transaction behavior.
- Run session-focused tests for open/save-as/clear changes.
