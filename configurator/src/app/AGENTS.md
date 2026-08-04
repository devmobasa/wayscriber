# AGENTS.md

## Scope
- Applies to configurator app state, updates, the GTK shell and its pages, side effects, daemon setup, search, and session catalog operations.
- Parent guidance here covers sibling files such as `session_catalog.rs`; child guides under `session_catalog/` should cover helper internals only.

## Architecture
- `state.rs` owns top-level app state.
- `update/` handles `Message` variants and returns `Vec<Effect>`.
- `component.rs` owns the GTK shell and is the only place effects run; `pages/` builds the sidebar pages from state.
- `chrome.rs` owns the controls whose widget depends on the libadwaita API floor, as `cfg` twins inside that one file.
- `dialog.rs` owns confirmation presentation lifecycle: the channel-neutral identity reducer that decides when a question goes up and comes down, and the response-to-message mapping both channels answer through. It holds no widget types.
- `io.rs`, `daemon_setup/`, `session_catalog.rs`, `session_catalog/`, `search/`, and `effects.rs` perform side-effecting or app-wide work.

## Invariants
- Do not do file/process work directly from page code.
- Keep `adw-modern` twins in `chrome.rs` only: page bodies carry no feature `cfg`, both twins emit the same message with the same payload, each owns its blocked programmatic write, and no Rust source file may be reachable only under the feature (the source-coverage matrix has no modern lane).
- Keep confirmation presentation driven by `dialog.rs` in both channels, so the reducer stays live and tested under the baseline lane. One owner holds each presented confirmation and is reached through `&mut` only. A reconcile close silences the channel's response handler before closing, and emits no message; presentation follows accepted model state, never a request the model refused.
- Preserve non-blocking effect behavior and explicit validation feedback.
- Run synchronous filesystem, locking, and process work from effect commands through `blocking_jobs`.
  One logical operation must use one adapter call; never nest adapter jobs.
- Session catalog operations must preserve lock checks, artifact movement, primary-file behavior, catalog collision handling, and rollback.
- Daemon setup behavior must stay aligned with daemon runtime, shared service/shortcut helpers, and packaging service files.

## Coupled Changes
- App changes may require model, message, page, search, docs, and tests updates.
- Daemon setup changes may require `src/daemon/`, `src/systemd_user_service.rs`, `src/shortcut_hint.rs`, `src/paths/`, and `packaging/wayscriber.service`.
- Session catalog changes may require `src/session/` and `src/paths/` updates.

## Validation
- Add focused tests near update/search/session catalog helpers where possible.
- Run `cargo test -p wayscriber-configurator` for app behavior changes.
- Manually run the configurator only when launching a foreground app is explicitly acceptable.
