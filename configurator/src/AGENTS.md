# AGENTS.md

## Scope
- Applies to configurator source code under `configurator/src/`.

## Architecture
- `main.rs` starts the Iced app.
- `messages.rs` defines the top-level message surface.
- `app/` owns state, update handlers, views, subscriptions, side effects, search, daemon setup, and session catalog workflows.
- `models/` owns draft/editable data and conversion to/from core wayscriber types.
- `test_env.rs` and `test_temp.rs` provide isolated test helpers.

## Invariants
- Keep message routing explicit and centralized.
- Keep view code side-effect free; route I/O and process work through update tasks or app-side helper modules.
- Preserve `default-features = false` use of the core crate.

## Coupled Changes
- Configurator changes often mirror main crate config, action metadata, keybindings, session, daemon setup, and path behavior.
- Search and labels must stay aligned with visible configuration sections and option names.

## Validation
- Use targeted tests under `configurator/src/models/`, `configurator/src/app/search/`, or `configurator/src/app/update/`.
- Run `cargo test -p wayscriber-configurator` for broad configurator changes.
