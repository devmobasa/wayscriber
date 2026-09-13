# AGENTS.md

## Scope
- This file applies to the whole wayscriber workspace.
- wayscriber is a Rust desktop annotation app for Wayland compositors.
- The workspace root crate is `wayscriber`; `configurator/` is a separate GTK4/libadwaita (Relm4) desktop configurator crate.
- Both crates use Rust 1.98.1 and edition 2024; `rust-toolchain.toml` pins the development toolchain.
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

## Code Readability
- Let `rustfmt` own mechanical Rust layout. Use standard Rust spacing: `call(value)`, `name: Type`, `T: Trait`, and `left + right`; omit spaces inside parentheses and around `::`, `.`, `?`, and ranges, and omit unnecessary control-flow parentheses.
- Separate logical steps within functions with one blank line: validation and early exits, setup, execution or mutation, state and effect publication, and returning or mapping the result. Keep closely related statements together; add spacing when the purpose changes rather than after every statement.
- Keep a `let ... else`, `match`, or parsed value with its immediate validation and error path. Use `?` for ordinary error propagation instead of expanding it solely to imitate another language's guard style.
- Let `rustfmt` decide wrapping, same-line braces, `} else {`, indentation, and multiline collection or argument layout. Do not use formatter-skip attributes to force a competing layout.
- Apply this grouping consistently to production Rust and tests. Preserve intentional blank lines while editing and formatting.
- Review changed functions after `cargo fmt`. Formatter success verifies mechanical layout; also verify visually that logical chunks are clearly separated and easy to scan.
- Apply these rules to new and materially changed code. Do not reformat unrelated existing code solely to adopt them.

## Coupled Changes
- Config changes often require `config.example.toml`, `docs/CONFIG.md`, configurator models/views, schema behavior, and tests.
- Tool/action/keybinding changes often require config defaults, action metadata, help/command UI, toolbar UI, configurator labels/search, docs, and tests.
- Daemon, service, shortcut, path, and packaging changes often need updates across `src/daemon/`, `src/paths/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `configurator/src/app/daemon_setup/`, and `packaging/`.

## Validation
- Full local CI is `./tools/lint-and-test.sh`.
- That script runs version/package checks, `cargo fmt --all -- --check`, clippy with all targets/features, all-feature tests, and no-default-feature tests.
- Run `git diff --check` before handoff. When relevant files are untracked and the index must stay unchanged, check those files directly as well.
- For docs-only `AGENTS.md` edits, make new files visible to Git before whitespace checks, for example `rg --files --hidden -g AGENTS.md -0 | xargs -0 git add -N --` followed by `git diff --check`.
- On PowerShell, use `rg --files --hidden -g AGENTS.md | ForEach-Object { git add -N -- $_ }` followed by `git diff --check`.
- If you do not want to alter the index, run an explicit trailing-whitespace check across the untracked `AGENTS.md` files instead of relying on plain `git diff --check`.
