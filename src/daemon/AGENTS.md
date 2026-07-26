# AGENTS.md

## Scope
- Applies to daemon lifecycle, toggle protocol, runtime files, tray integration, setup helpers, global shortcuts, overlay process control, and daemon tests.

## Architecture
- `core.rs`, `control.rs`, and `types.rs` own daemon state and toggle/control behavior.
- `overlay/` owns overlay process spawn/control.
- `tray/` owns tray integration and shortcut hint I/O.
- `setup.rs` and `global_shortcuts.rs` support daemon setup workflows.
- `update_watch.rs` owns the background update notice: it publishes to `TrayStatusShared`
  and sends at most one desktop notification per release, and installs nothing.

## Invariants
- Preserve single-instance locking, stale runtime cleanup, duplicate-toggle suppression, typed daemon toggle responses, and named-session switching rules.
- The update watcher must stay opt-outable, must never notify over an active overlay, and must never install anything; see `src/update_check/AGENTS.md`.
- Keep runtime path behavior aligned with `src/paths/`.
- Do not change service/shortcut behavior without checking configurator daemon setup and packaging.

## Coupled Changes
- Daemon changes may affect `src/paths/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `configurator/src/app/daemon_setup/`, `packaging/wayscriber.service`, and setup docs.

## Validation
- Run daemon-focused tests for lifecycle/toggle changes.
- Run full local CI for process, lock, service, or feature-gate changes.
