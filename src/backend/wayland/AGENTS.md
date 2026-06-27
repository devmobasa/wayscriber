# AGENTS.md

## Scope
- Applies to the Wayland backend runtime under `src/backend/wayland/`.
- Covers central split-module files such as `state.rs` and `session.rs` plus child runtime subtrees.

## Architecture
- `backend/` owns startup, setup, event loop phases, signals, tray integration, capture polling, rendering, and session save on exit.
- `handlers/` translates Wayland/Smithay callbacks into state and input calls.
- `state.rs` is the central live runtime state root; `state/` contains state helpers for boards, buffers, damage, clipboard paste, toolbar plumbing, zoom, onboarding, and export handoff.
- `toolbar/` owns runtime toolbar layout, hit testing, rows, surfaces, rendering, and widgets.
- `clipboard/`, `frozen/`, `zoom/`, and runtime session helpers integrate external compositor, clipboard, capture, and persistence behavior.

## Invariants
- Preserve Wayland object lifetimes, dispatch order, frame callbacks, surface lifecycle, output identity, damage tracking, and feature gates.
- Keep protocol handlers thin; route durable business logic into state/input/capture/session owners.
- Do not block event-loop dispatch or rendering with file, process, clipboard, or capture work.
- Child guides under split-module directories must not be used to govern sibling `*.rs` files; keep shared rules here or in the immediate parent.

## Coupled Changes
- Tablet behavior must stay coherent with `tablet-input`.
- Portal capture and URI/file handling must stay coherent with `portal` and `dbus`.
- Tray behavior must stay coherent with `tray`, daemon runtime, configurator daemon setup, and packaging service files.
- Clipboard file URI behavior must stay aligned with shared `src/file_uri.rs`.

## Validation
- Use focused tests near changed state/session/toolbar helpers when available.
- Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` for protocol/runtime changes.
- Run full local CI for broad event-loop, feature-gate, or runtime lifecycle changes.
