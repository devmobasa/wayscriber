# AGENTS.md

## Scope
- Applies to the Wayland clipboard module under `clipboard/`.
- This directory is not a split module; `clipboard/mod.rs` is covered by this file.

## Architecture
- `image.rs` and `file_list.rs` build clipboard payloads.
- `transfer.rs` owns Wayland clipboard transfer helpers.
- `system.rs` and `system/command.rs` provide external command fallback behavior.

## Invariants
- Treat file URI and external command inputs as trust boundaries.
- Keep clipboard error reporting and cancellation behavior explicit.
- Preserve compatibility between image payloads, URI-list payloads, and session paste behavior in `src/backend/wayland/state/clipboard.rs`.

## Coupled Changes
- URI-list behavior must stay aligned with shared `src/file_uri.rs`.
- Clipboard paste changes may affect input selection behavior and session paste tests.
- System command behavior may affect packaging/runtime dependencies and docs.

## Validation
- Add or update tests under `clipboard/transfer/`, `clipboard/system/`, or state clipboard tests.
- Use targeted cargo tests for clipboard behavior.
