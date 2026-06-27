# AGENTS.md

## Scope
- Applies to top-level application orchestration under `src/app/`.

## Architecture
- Owns mode routing, environment checks, session CLI handling, and usage output.
- Delegates daemon lifecycle to `src/daemon/` and active overlay runtime to `src/backend/`.

## Invariants
- Keep CLI-visible behavior aligned with docs and integration tests.
- Do not duplicate daemon, backend, session, or config ownership in this layer.
- Preserve Wayland environment checks for modes that require a compositor session.

## Coupled Changes
- CLI/mode changes may require `src/cli.rs`, `tests/cli.rs`, `README.md`, `docs/SETUP.md`, and daemon/session docs.

## Validation
- Run CLI integration tests for user-visible CLI changes.
- Run full local CI for broad mode routing changes.
