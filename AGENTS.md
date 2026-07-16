# AGENTS.md

## Scope
- This file applies to the whole wayscriber workspace.
- wayscriber is a Rust desktop annotation app for Wayland compositors.
- The workspace root crate is `wayscriber`; `configurator/` is a separate Iced desktop configurator crate.
- Both crates use Rust 1.95 and edition 2024.
- The workspace default member is only the root crate; use `-p wayscriber-configurator` or `--workspace` when configurator coverage is needed.

## Architecture
- `src/main.rs` is a thin wrapper around the public `wayscriber::run_from_env()` entry facade.
- `src/lib.rs` owns the canonical application module graph: reusable modules remain public for tests and the configurator, while runtime modules stay private behind the entry facade.
- Core domains are stable shared values, backend/Wayland runtime, input state, drawing data/rendering, overlay UI, capture, config, session persistence, daemon lifecycle, canvas export, and configurator UI.
- Start with `README.md`, `CONTRIBUTING.md`, `docs/codebase-overview.md`, `docs/CONFIG.md`, and `configurator/README.md` when changing behavior.

## Invariants
- Preserve foreground and fullscreen safety: do not launch, foreground, focus, or interact with another app/window, especially fullscreen apps, without explicit user permission.
- Treat `cargo run`, `wayscriber --active`, `wayscriber --daemon`, `wayscriber-configurator`, and manual UI checks as potentially foreground/focus-affecting unless proven otherwise.
- Preserve feature-gate intent for `tablet-input`, `dbus`, `portal`, `tray`, and `config-schema`.
- Do not edit generated/build output such as `target/`.
- Preserve existing Rust module style. This repo intentionally mixes `foo.rs`, `foo/`, and `foo/mod.rs`.
- For Rust split modules, an `AGENTS.md` in `foo/` does not apply to sibling `foo.rs`; put shared rules in the parent guide.

## Coupled Changes
- Config changes often require `config.example.toml`, `docs/CONFIG.md`, configurator models/views, schema behavior, and tests.
- Tool/action/keybinding changes often require config defaults, action metadata, help/command UI, toolbar UI, configurator labels/search, docs, and tests.
- Daemon, service, shortcut, path, and packaging changes often need updates across `src/daemon/`, `src/paths/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `configurator/src/app/daemon_setup/`, and `packaging/`.

## Validation
- Full local CI is `./tools/lint-and-test.sh`.
- That script runs version/package checks, `cargo fmt --all -- --check`, clippy with all targets/features, all-feature tests, and no-default-feature tests.
- For docs-only `AGENTS.md` edits, make new files visible to Git before whitespace checks, for example `rg --files --hidden -g AGENTS.md -0 | xargs -0 git add -N --` followed by `git diff --check`.
- On PowerShell, use `rg --files --hidden -g AGENTS.md | ForEach-Object { git add -N -- $_ }` followed by `git diff --check`.
- If you do not want to alter the index, run an explicit trailing-whitespace check across the untracked `AGENTS.md` files instead of relying on plain `git diff --check`.
