# AGENTS.md

## Scope
- This file applies to the whole wayscriber workspace.
- wayscriber is a Rust desktop annotation app for Wayland compositors.
- The workspace root crate is `wayscriber`; `configurator/` is a separate GTK4/libadwaita (Relm4) desktop configurator crate.
- Both crates use Rust 1.99.1 and edition 2024; `rust-toolchain.toml` pins the development toolchain.
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

## Rust Style
- Rust code follows the [Google Rust style guide](https://google.github.io/styleguide/rustguide.html) as application code. Its rules that Clippy can check are workspace lints in `Cargo.toml`, configured in `clippy.toml`. Fix a warning rather than suppressing it. When a site truly cannot comply, use `#[expect(lint, reason = "...")]` with the concrete constraint, never `#[allow]`.
- Each `use` statement imports from one module: `use foo::bar::{Bar, Baz};`, not `use foo::{Foo, bar::Bar};`. `tests/repository_guards/use_statements.rs` checks this because rustfmt's `imports_granularity` is unstable.
- Do not use glob imports, except `use super::*` in test modules and third-party preludes.
- Import type and trait names and use them unqualified; call functions through their module (`fs::read`). Keep ambiguous names such as `io::Error` and `fmt::Result` module-qualified. A gated import carries the same `#[cfg(...)]` as the code that uses it.
- Borrow instead of cloning or taking ownership (`&str`, `&[T]`, `Option<&T>`). Replace more than three bool parameters with enums or an options struct.
- Use `expect` with a reason instead of `unwrap` in production code. Give assertions messages that explain the invariant, and document panics in `# Panics`.
- Never reach inside a poisoned lock. Use `expect` when the critical section cannot panic, or a fallback that leaves the data alone. Tests that serialize on process-wide state use `test_env::lock()` or a `test_env::Lease`.
- `catch_unwind` is reserved for the reviewed worker-isolation sites that carry an `#[expect]` reason. Tests observe panics through a joined scoped thread.
- Getters are `foo()`/`foo_mut()`, a fallible constructor is `try_new`, and type lifetimes have descriptive names. Every `unsafe` block has a `// SAFETY:` comment that says why it is sound.
- Convert numbers losslessly with `From` (`f64::from(width)`) wherever it can be called; `const` initializers keep `as`. Take the middle of two values with `midpoint` (`f64::midpoint(a, b)`), which cannot overflow.
- Inline named format arguments: `format!("{name}: {err}")`.
- Use `map_or`, `map_or_else`, `is_some_and`, or `is_ok_and` instead of `map(..).unwrap_or(..)`. Use `let ... else` for a `match` that only unwraps or leaves, and `if let ... else` for a `match` with one pattern and a fallback.
- In doc comments, put identifiers and protocol names in backticks (`wl_output`). Product names that docs write plainly go in `doc-valid-idents` in `clippy.toml`.
- Take raw pointers with `&raw const`/`&raw mut`, change pointer constness with `cast_const`/`cast_mut`, and write the anonymous lifetime in paths (`Formatter<'_>`). Give every `#[ignore]` a reason: `#[ignore = "needs a running Wayland compositor"]`.
- Clippy pedantic lints are enabled one at a time in `Cargo.toml`, never as the whole group, so a toolchain update does not turn on new ones.

## Coupled Changes
- Config changes often require `config.example.toml`, `docs/CONFIG.md`, configurator models/views, schema behavior, and tests.
- Tool/action/keybinding changes often require config defaults, action metadata, help/command UI, toolbar UI, configurator labels/search, docs, and tests.
- Daemon, service, shortcut, path, and packaging changes often need updates across `src/daemon/`, `src/paths/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `configurator/src/app/daemon_setup/`, and `packaging/`.
- Dependency changes must satisfy `deny.toml`. A new license or duplicate version needs an entry there with its reason.

## Validation
- Full local CI is `./tools/lint-and-test.sh`. It needs the .NET SDK selected by `global.json`. Without it, `cargo test` still runs the Rust source guards in `tests/repository_guards` (process sites, config writers, shared dependencies, use statements, no Python), but not the C# checks, including the source-coverage check that finds `.rs` files Cargo never compiles.
- That script runs the C# repository checks (assets, version, nixpkgs recipe, source coverage, legacy tools), the C# tool tests (which also run the packaging shell contracts), `cargo fmt --all -- --check`, clippy with all targets/features, all-feature tests, and no-default-feature tests.
- Run `git diff --check` before handoff. When relevant files are untracked and the index must stay unchanged, check those files directly as well.
- For docs-only `AGENTS.md` edits, make new files visible to Git before whitespace checks, for example `rg --files --hidden -g AGENTS.md -0 | xargs -0 git add -N --` followed by `git diff --check`.
- On PowerShell, use `rg --files --hidden -g AGENTS.md | ForEach-Object { git add -N -- $_ }` followed by `git diff --check`.
- If you do not want to alter the index, run an explicit trailing-whitespace check across the untracked `AGENTS.md` files instead of relying on plain `git diff --check`.
