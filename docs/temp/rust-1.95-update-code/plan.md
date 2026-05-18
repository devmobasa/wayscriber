# Rust 1.95 Follow-up Code Update Plan

Status: deferred. The Rust 1.95 compatibility bump should stay limited to MSRV/toolchain/docs unless CI exposes a required fix. This plan is for later code cleanup once the team is comfortable requiring Rust 1.95 everywhere.

## Baseline

- `rustc 1.95.0` and `cargo 1.95.0` build the current workspace without source changes.
- Verified locally before the compatibility bump:
  - `cargo check --workspace --all-features`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace --all-features`
  - `cargo test --workspace --no-default-features`
  - `cargo fmt --all -- --check`
- No custom JSON target specs, unstable `#[feature]` gates, or `$crate` self-imports were found.
- The codebase has many feature/platform `cfg` blocks and many small nested `if let` patterns. Those are the main places where Rust 1.95 may improve clarity.

## Guardrails

- Do not do a repo-wide syntax migration.
- Change one concern at a time and keep each PR behavior-preserving.
- Prefer the existing module boundaries. If a file is already large, use the cleanup as a chance to move toward concern-based modules rather than adding more code.
- Keep public config/session formats unchanged unless a separate feature requires it.
- After each slice, run at least `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and the narrow affected tests. Before merge, run `./tools/lint-and-test.sh`.

## Candidate Updates

### 1. Use `cfg_select!` for dense cfg dispatch

Rust 1.95 stabilizes `cfg_select!`. Use it only where it removes duplicated function bodies or closely related cfg branches.

Good candidates:
- `src/time_utils.rs`: Unix/non-Unix time conversion branches and target-pointer-width split.
- `src/file_uri.rs`: Unix/non-Unix path conversion.
- `src/session/lock.rs`: Unix/non-Unix lock behavior.
- `src/backend/wayland/state/core/accessors.rs`: tablet/non-tablet state accessors.
- `src/config/types/mod.rs` and `src/config/core.rs`: tablet-gated config shape.

Approach:
1. Prototype on one small module, likely `src/session/lock.rs` or `src/file_uri.rs`.
2. Compare readability against the current `#[cfg]` attributes.
3. Keep module-level cfg attributes when they are clearer than `cfg_select!`.
4. Run both feature matrices, especially `--all-features` and `--no-default-features`, because tablet and portal/tray gates are easy to regress.

### 2. Use `if let` guards in match arms where they flatten control flow

Rust 1.95 stabilizes `if let` guards. Prefer this only when it makes the match's decision table easier to read.

Good candidates:
- `build.rs`: nested `if let` logic in git-dir resolution.
- `src/tray_action.rs`: queued action parsing/cleanup paths.
- `src/daemon/tray/helpers.rs`: command spawning, runtime selection, and config/session result handling.
- `src/input/state/core/menus/layout.rs`: nested layout/index/entry checks.
- `src/input/state/core/menus/commands.rs`: repeated anchor/layout fallback decisions.

Approach:
1. Start with non-runtime-critical code such as `build.rs`.
2. Replace only a nested `match`/`if let` cluster when the guard expresses the condition directly.
3. Avoid clever guards with side effects or long expressions.
4. Keep tests focused on the touched behavior.

### 3. Consider `Vec::push_mut` and `Vec::insert_mut` for immediate mutation

Rust 1.95 adds stable mutable references from vector insertion APIs. Use them only where code currently inserts then immediately re-finds or mutates the inserted item.

Potential search areas:
- UI/model builders that construct vectors of toolbar/menu entries.
- Session/history code around inserted shapes or actions.
- `src/input/state/core/command_palette/search.rs`, where recent command insertion may benefit only if follow-up mutation appears later.
- `src/draw/frame/history/frame/apply.rs`, where shape insertion is sensitive; change only with regression tests.

Approach:
1. Search for `push(...);` followed by `last_mut`, `len() - 1`, or `get_mut`.
2. Search for `insert(...);` followed by `get_mut` or index-based mutation.
3. Prefer no change if the current code is already clearer or if the inserted value is not mutated.
4. Add or keep regression coverage for shape ordering/history behavior before touching frame insertion paths.

### 4. Revisit atomic state updates

Rust 1.95 adds atomic `update` and `try_update`. Current atomic usage appears simple (`fetch_add`, bool flags, small overrides), so this is likely a no-op.

Potential files:
- `src/tray_action.rs`
- `src/session_override.rs`
- daemon and backend state that use atomics for lifecycle flags

Approach:
1. Search for manual `compare_exchange` loops. If none exist, skip this slice.
2. Do not replace simple `store`, `load`, or `fetch_add`; that would reduce clarity.
3. If a compare/exchange loop is introduced later, prefer `try_update` when the failure path matters.

### 5. Audit new Clippy lints without churn

Clippy 1.95 adds default lints including `manual_checked_ops`, `manual_take`, and `disallowed_fields`. Current `-D warnings` passed, so there is no required work.

Approach:
1. Run Clippy after dependencies are refreshed in future work.
2. Treat new warnings as local cleanups, not a style migration.
3. Prefer `#[expect(..., reason = "...")]` for intentional exceptions.

### 6. Keep performance-only APIs measured

Rust 1.95 includes `core::hint::cold_path`. Do not add it broadly.

Potential areas only after profiling:
- Rare config parse/reporting error paths.
- Capture failure/reporting paths.
- Session corruption fallback paths.

Approach:
1. Add only after a measured hot path shows code layout matters.
2. Keep the hint near the rare branch, with a short reason.
3. Avoid using it as documentation for ordinary error handling.

## Suggested PR Sequence

1. `build.rs` and one small platform module: prove `if let` guards or `cfg_select!` improve clarity.
2. Tablet cfg cleanup: only if `cfg_select!` meaningfully reduces duplicate tablet/non-tablet accessors.
3. History/vector insertion cleanup: only if `push_mut` or `insert_mut` removes indexing or lookup without changing ordering.
4. Atomic audit: likely close as "no change needed" unless compare/exchange loops appear.
5. Final Clippy pass after dependency updates.

## Nix Follow-up

The flake currently relies on the pinned `nixpkgs` Rust toolchain. When Nix is available, run:

```bash
nix flake update nixpkgs
nix develop --command sh -lc 'rustc --version && cargo --version'
nix build .#wayscriber .#wayscriber-configurator
```

If the updated `nixpkgs` still does not provide Rust 1.95+, add an explicit Rust 1.95 toolchain override in `flake.nix` rather than relying on an older default `pkgs.rustPlatform`.
