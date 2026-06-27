# AGENTS.md

## Scope
- Applies to configurator daemon setup helpers under `configurator/src/app/daemon_setup/`.

## Architecture
- Owns setup commands, Hyprland shortcut integration, systemd user service behavior, and shortcut helper UI flows.

## Invariants
- Keep daemon setup aligned with daemon runtime behavior, shared service/shortcut helpers, and packaging service files.
- Avoid blocking UI update paths; use tasks for process/file operations.
- Preserve clear user-facing errors for setup failures.

## Coupled Changes
- Daemon setup changes may affect `src/daemon/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `src/paths/`, `packaging/wayscriber.service`, setup docs, and tests.

## Validation
- Add focused tests for command/service/shortcut helpers where possible.
- Run configurator tests for daemon setup behavior changes.
