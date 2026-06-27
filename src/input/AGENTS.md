# AGENTS.md

## Scope
- Applies to input events, tools, board modes, hit testing, tablet glue, board operations, and `InputState`.
- Parent guidance here covers split roots such as `boards.rs` plus child modules under `boards/`.

## Architecture
- Backend code forwards pointer, keyboard, tablet, and touch events into input state.
- `InputState` owns action dispatch, mouse/keyboard handling, interaction routing, panels, selection, text editing, highlight state, and pending backend actions.
- `tool/` owns tool kind/catalog/profile/settings/drag behavior.
- `boards.rs` and `boards/` own board/page identity, ordering, naming, color, mapping, and operations.

## Invariants
- Backend code should drain pending backend actions instead of directly mutating drawing, capture, or session state.
- Keep drawing mutations behind `InputState` methods so dirty tracking, undo, and history remain coherent.
- Preserve panel focus, command-palette dispatch, text input lifecycle, selection transforms, board/page identity, and session preflight behavior.

## Coupled Changes
- Tool changes must update config defaults, action metadata, keybinding actions/default maps, toolbar UI, backend render behavior, docs, tests, configurator field models, configurator drawing views, and configurator search labels.
- Board changes may affect session snapshots, board picker UI, toolbar board/page controls, config defaults, and tests.

## Validation
- Add focused tests under `src/input/state/tests/`, `src/input/boards/`, or `src/input/tool/` for behavior changes.
- Use targeted `cargo test` filters for input state, boards, selection, text input, or tools.
- Run full local CI for changes that alter action dispatch, tool semantics, or board persistence.
