# AGENTS.md

## Scope
- Applies to the `wayscriber-configurator` crate.
- This is a separate Iced desktop crate that reuses core `wayscriber` config and session types.

## Architecture
- `configurator/src/main.rs` is the binary entry point.
- `configurator/src/messages.rs` owns top-level UI messages.
- `configurator/src/app/` owns app state, update logic, views, daemon setup, search, subscriptions, I/O, and session catalog operations.
- `configurator/src/models/` owns editable models, parsing, validation, conversion to core config/session types, labels, search data, and reusable fields.

## Invariants
- Keep `wayscriber` dependency usage compatible with `default-features = false`; do not pull portal D-Bus or tray runtime behavior into the configurator binary.
- Views should emit messages and render state; file/process work belongs in update tasks or side-effect modules.
- Invalid user input should remain visible and actionable instead of being silently coerced or dropped.

## Coupled Changes
- Main config changes often require configurator draft models, setters, `to_config/`, views, updates, labels, search terms, docs, and tests.
- Daemon setup changes must stay aligned with `src/daemon/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, and `packaging/wayscriber.service`.
- Session catalog changes must stay aligned with `src/session/` lock, artifact, catalog, and path behavior.

## Validation
- Run `cargo test -p wayscriber-configurator` or targeted configurator tests for model/update changes.
- Use `cargo run -p wayscriber-configurator` for manual UI checks when user-approved foreground app launch is acceptable.
- Run full local CI for workspace-level config/session changes.
