# AGENTS.md

## Scope
- Applies to integration tests under `tests/`.

## Architecture
- `tests/cli.rs` covers CLI behavior.
- `tests/ui.rs` covers UI smoke/integration behavior that can run without an ungated visible overlay.

## Invariants
- Tests should not launch visible Wayland overlays, steal focus, or interact with foreground/fullscreen apps by default.
- Use isolated temp dirs and environment variables for CLI, config, path, and session behavior.
- Keep output assertions specific enough to catch regressions without depending on noisy logs.

## Coupled Changes
- CLI changes may require `tests/cli.rs`, docs, and usage text updates.
- UI or rendering smoke changes may require fixtures or focused module tests in `src/`.

## Validation
- Run targeted integration tests for touched behavior.
- Run full local CI for broad CLI, config, session, or workspace changes.
