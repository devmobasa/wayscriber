# AGENTS.md

## Scope
- Applies to `InputState` internals under `src/input/state/`.

## Architecture
- `actions/` owns action dispatch and key press/release behavior.
- `core/` owns board state, history, selection, panels, properties, command palette, board picker, utilities, and session preflight.
- `mouse/`, `interaction/`, and `highlight/` own pointer/mouse routing, interaction adapters, and highlight state.
- `tests/` owns focused input state tests.

## Invariants
- Keep direct drawing mutations behind `InputState` methods so dirty tracking, undo, and history remain coherent.
- Preserve pending backend action boundaries; backend code should drain actions rather than duplicating side effects.
- Preserve text input lifecycle, panel focus, selection transforms, command palette dispatch, and session preflight behavior.

## Coupled Changes
- Input state changes may affect `src/draw/`, `src/ui/`, backend Wayland handlers/state, session, capture/export, toolbar model/apply, config actions, and tests.

## Validation
- Add focused tests under `src/input/state/tests/` for behavior changes.
- Use targeted tests for menus, selection, text input, board picker, properties, or actions.
