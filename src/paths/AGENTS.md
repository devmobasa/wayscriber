# AGENTS.md

## Scope
- Applies to XDG and runtime path resolution helpers under `src/paths/`.
- Covers config/session paths plus daemon command files, tray action paths, pid files, log paths, overlay locks, daemon locks, runtime/data/config directories, and tests.

## Architecture
- Path helpers are shared by config loading, session storage, daemon runtime files, configurator workflows, and packaging/user-service behavior.
- Environment overrides and XDG directory behavior should stay centralized here rather than duplicated in callers.

## Invariants
- Preserve path safety, XDG semantics, environment override behavior, and cross-platform assumptions already encoded by tests.
- Do not broaden trusted filesystem locations without checking daemon/session/config callers.
- Keep runtime path changes compatible with daemon single-instance locks, command queues, tray actions, and overlay process management.

## Coupled Changes
- Path changes may affect `src/config/`, `src/session/`, `src/daemon/`, `src/systemd_user_service.rs`, configurator session/daemon setup, docs, and packaging service behavior.

## Validation
- Add or update `src/paths/tests.rs` for path behavior changes.
- Run targeted path/session/daemon tests when changing runtime paths.
- Run full local CI for broad path resolution changes.
