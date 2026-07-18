# AGENTS.md

## Scope
- Applies to configurator app state, updates, views, side effects, daemon setup, search, subscriptions, and session catalog operations.
- Parent guidance here covers sibling files such as `session_catalog.rs`; child guides under `session_catalog/` should cover helper internals only.

## Architecture
- `state.rs` owns top-level app state.
- `update/` handles `Message` variants and returns `iced::Task<Message>`.
- `view/` builds Iced UI from state.
- `io.rs`, `daemon_setup/`, `session_catalog.rs`, `session_catalog/`, `search/`, and `subscription.rs` perform side-effecting or app-wide work.

## Invariants
- Do not do file/process work directly from view code.
- Preserve non-blocking task behavior and explicit validation feedback.
- Run synchronous filesystem, locking, and process work from Iced tasks through `blocking_jobs`.
  One logical operation must use one adapter call; never nest adapter jobs.
- Session catalog operations must preserve lock checks, artifact movement, primary-file behavior, catalog collision handling, and rollback.
- Daemon setup behavior must stay aligned with daemon runtime, shared service/shortcut helpers, and packaging service files.

## Coupled Changes
- App changes may require model, message, view, search, docs, and tests updates.
- Daemon setup changes may require `src/daemon/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `src/paths/`, and `packaging/wayscriber.service`.
- Session catalog changes may require `src/session/` and `src/paths/` updates.

## Validation
- Add focused tests near update/search/session catalog helpers where possible.
- Run `cargo test -p wayscriber-configurator` for app behavior changes.
- Manually run the configurator only when launching a foreground app is explicitly acceptable.
