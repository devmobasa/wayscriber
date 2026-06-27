# AGENTS.md

## Scope
- Applies only to helper modules under `configurator/src/app/session_catalog/`.
- The sibling root `configurator/src/app/session_catalog.rs` is governed by `configurator/src/app/AGENTS.md`.

## Architecture
- Helper modules support inactive-session duplicate, move, and tests.
- They coordinate with core session catalog, artifact, lock, and path behavior.

## Invariants
- Preserve runtime lock checks, primary-file behavior, catalog collision checks, artifact movement, rollback behavior, and error reporting.
- Do not mutate inactive sessions without validating target paths and lock state.

## Coupled Changes
- Session catalog helper changes may affect `src/session/`, `src/paths/`, configurator app state/update/view, and tests.

## Validation
- Add focused tests under this subtree for duplicate/move behavior.
- Run configurator tests for session catalog changes.
