# AGENTS.md

## Scope
- Applies to the main `wayscriber` crate under `src/`.
- Covers parent guidance for file-level modules and sibling Rust split-module roots.

## Architecture
- `src/main.rs` is a thin wrapper that returns `wayscriber::run_from_env()`.
- `src/lib.rs` owns logging/error wiring and the canonical module graph; its public entry facade routes CLI outcomes through `src/app/` into `src/daemon/` or the active Wayland backend.
- Reusable modules remain public for integration tests and the configurator, while runtime modules are private library implementation details.
- Major domains are `backend`, `input`, `draw`, `ui`, `capture`, `config`, `session`, `daemon`, `canvas_export`, `paths`, `toolbar_icons`, `render_profiles`, and `runtime_capabilities`.

## Invariants
- Keep Wayland/runtime side effects in backend/about-window code; keep reusable data, validation, rendering helpers, and path logic in library modules.
- Preserve feature gates when moving code between binary-private and library-exported modules.
- Shared helpers such as `src/file_uri.rs`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `src/ui.rs`, `src/bin/dump_config_schema.rs`, and `src/toolbar_icons/` need parent-level coupling rules here because sibling child guides do not apply to them.
- `src/file_uri.rs` is a shared URI trust boundary used by clipboard URI-list paste and portal capture reading.
- `src/systemd_user_service.rs` and `src/shortcut_hint.rs` couple daemon runtime, configurator daemon setup, shortcut backend selection, and packaging service behavior.

## Coupled Changes
- Schema dump changes in `src/bin/` must stay aligned with `src/config/schema.rs` and the `config-schema` feature.
- Toolbar icon changes must stay aligned with toolbar UI, backend toolbar rendering, action metadata, and visual tests where applicable.
- UI module-root changes in `src/ui.rs` must stay aligned with `src/ui/` child modules and input/backend render callers.

## Validation
- Use focused `cargo test` filters for touched modules when possible.
- Run `cargo fmt --all -- --check` for Rust edits.
- Run `./tools/lint-and-test.sh` for broad behavior, feature-gate, or workspace-level changes.
