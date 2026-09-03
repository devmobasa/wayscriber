# AGENTS.md

## Scope
- Applies to `InputState` internals under `src/input/state/`.

## Architecture
- `actions/` owns action dispatch and key press/release behavior.
- `core/toolbar/apply/` owns `InputState` mutation for toolbar events; toolbar models, snapshots, and event values remain under `src/ui/toolbar/`.
- `core/` owns board state, history, selection, panels, properties, command palette, board picker, utilities, and session preflight. Panels with their own lifecycle own a state type (`core/font_picker/state.rs`, `core/command_palette/state.rs`, `core/help_overlay/state.rs`, `core/board_picker/panel.rs`, `core/menus/context_menu.rs`, `core/color_picker_popup/panel.rs`, `core/radial_menu/panel.rs`, `core/status_hud/state.rs`, `core/zoom_chip/state.rs`, `core/properties/state.rs`, `core/tour.rs::TourState`); code outside `input::state` reads them through accessors, not fields. Shared modal keyboard-repeat timing belongs to `core/key_repeat.rs`, not to an individual panel's input handler. `core/style.rs` owns drawing-style mutation and preset/session conversion, `core/presets.rs` owns preset slot lifecycle, and `core/history_limits.rs` owns undo retention and delayed playback scheduling. `core/text_editing.rs` owns text mode, asynchronous edit identity, IME composition, caret/selection edits, text-block pointer state, and existing-shape edit lifecycle; root wrappers retain dirty tracking, redraw, session, and backend-effect coordination. `core/selection.rs` owns selection membership, nudge-axis memory, and polygon click timing; `core/selection/clipboard.rs` owns local shape clipboard generations, publication state, paste request identity, and image-save fallback. `core/keymap.rs` owns action and sequence matching, rebind revisions, pointer-button consumption, drag-tool bindings, active pointer-drag identity, and shortcut capture; shared modifiers remain on `InputState`. `core/view.rs`, `core/pointer.rs`, and `core/index.rs` own view transforms, pointer bookkeeping, and canvas hit-test/index policy respectively, while `spotlight.rs::SpotlightWheelGesture` owns wheel-burst state; root wrappers retain cross-owner dirty, redraw, and board coordination. `core/base/ui_visibility.rs` groups the UI visibility preferences; `core/search.rs` holds the shared fuzzy scorer.
- `from_config.rs` is the only place configuration becomes an `InputState`; runtime code constructs through `InputState::from_config` and tests through `test_support::TestInputStateBuilder`.
- `mouse/`, `interaction/`, and `highlight/` own pointer/mouse routing, interaction adapters, and highlight state.
- `tests/` owns focused input state tests.

## Invariants
- Keep drawing-value mutation on `DrawingStyle`; `InputState` wrappers coordinate dirty tracking, undo, redraw, and session side effects.
- Keep text buffer, IME, text-clipboard identity, and existing-shape edit transitions on `TextEditing`; do not move font values out of `DrawingStyle`.
- Keep selection and shape-clipboard transitions on their owners. Pure per-variant translation and scaling belong to `Shape`; `InputState` coordinates frame mutation, undo, dirty regions, redraw, and backend effects.
- Keep keymap matching, sequence deadlines, rebind revisions, pointer-button consumption, drag bindings, and capture lifecycle on `Keymap`; root wrappers retain redraw and cross-owner cleanup.
- Keep transform math on `ViewState`, pointer bookkeeping on `PointerTracking`, Spotlight wheel-burst lifecycle on `SpotlightWheelGesture`, and hit-test cache/index policy on `CanvasIndex`; pass board offsets, frames, and frame guards into owners rather than giving them board access.
- Preserve pending backend action boundaries; backend code should drain actions rather than duplicating side effects.
- Preserve text input lifecycle, panel focus, selection transforms, command palette dispatch, and session preflight behavior.

## Coupled Changes
- Input state changes may affect `src/draw/`, `src/ui/`, backend Wayland handlers/state, session, capture/export, toolbar model/apply, config actions, and tests.

## Validation
- Add focused tests under `src/input/state/tests/` for behavior changes.
- Use targeted tests for menus, selection, text input, board picker, properties, or actions.
