# AGENTS.md

## Scope
- Applies only to child helpers under `src/input/boards/`.
- The sibling root `src/input/boards.rs` is governed by `src/input/AGENTS.md`.

## Architecture
- Owns board/page identity, mapping, naming, color, operations, and tests.

## Invariants
- Preserve stable identifiers, ordering, active-board transitions, configured defaults, and undo/history expectations.
- Keep board/page behavior coherent with session snapshots and board picker UI.

## Coupled Changes
- Board changes may affect `src/input/state/core/board*`, `src/session/`, `src/ui/board_picker/`, backend toolbar board/page rows, config board defaults, and tests.

## Validation
- Add focused board tests under `src/input/boards/` or `src/input/state/tests/`.
