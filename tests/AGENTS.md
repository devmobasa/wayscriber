# AGENTS.md

## Scope
- Applies to integration tests under `tests/`.

## Architecture
- `tests/cli.rs` covers CLI behavior.
- `tests/ui.rs` covers UI smoke/integration behavior that can run without an ungated visible overlay.
- `tests/repository_guards/` holds the source guards (process sites, config writers, shared dependencies, no Python). They read the checkout, not compiled items, and each guard keeps regression cases showing its forbidden escapes fail: most edit a copy of the checked-out `source::Tree`, the shared-dependency corpus builds small trees from `shared_dependency_fixtures.json`, and `no_python.rs` audits small in-memory file sets.

## Invariants
- Tests should not launch visible Wayland overlays, steal focus, or interact with foreground/fullscreen apps by default.
- Use isolated temp dirs and environment variables for CLI, config, path, and session behavior.
- Keep output assertions specific enough to catch regressions without depending on noisy logs.

## Coupled Changes
- A new process site, config writer, or shared-layer path must satisfy `tests/repository_guards` or update the guard's reviewed list with its reason.
- CLI changes may require `tests/cli.rs`, docs, and usage text updates.
- UI or rendering smoke changes may require fixtures or focused module tests in `src/`.

## Validation
- Run targeted integration tests for touched behavior.
- Run full local CI for broad CLI, config, session, or workspace changes.
