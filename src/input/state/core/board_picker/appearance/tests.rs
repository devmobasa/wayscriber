use super::*;
use crate::draw::{BLUE, RED};
use crate::input::state::test_support::TestInputStateBuilder;

fn editing() -> InputState {
    let mut input = TestInputStateBuilder::default().build();
    input.switch_board_force("whiteboard");
    input.open_board_picker_with_measurer(&crate::draw::TextMeasurer::default());
    input.board_picker_edit_color_selected_with_measurer(&crate::draw::TextMeasurer::default());
    input.clear_session_dirty();
    input
}

#[test]
fn appearance_preview_cancel_and_noop_never_change_board_or_session() {
    let mut input = editing();
    let original = BoardAppearance::from_spec(&input.boards.active_board().spec);
    let pen = input.color_for_tool(crate::input::Tool::Pen);
    input.board_appearance_palette(RED);
    input.board_picker.appearance.as_mut().unwrap().preview();
    assert_eq!(
        BoardAppearance::from_spec(&input.boards.active_board().spec),
        original
    );
    assert_eq!(input.color_for_tool(crate::input::Tool::Pen), pen);
    assert!(!input.is_session_dirty());
    input.board_appearance_key(Key::Escape);
    assert!(input.board_appearance_edit().is_none());
    assert!(!input.boards.active_board().appearance_explicit);
    input.board_picker_edit_color_selected_with_measurer(&crate::draw::TextMeasurer::default());
    assert!(input.apply_board_appearance());
    assert!(!input.is_session_dirty());
}

#[test]
fn appearance_apply_merges_untouched_fields_and_preserves_history_and_pen_for_grid_only() {
    let mut input = editing();
    let pen = input.color_for_tool(crate::input::Tool::Pen);
    let frame = input.boards.active_frame();
    let history = (
        frame.shapes.len(),
        frame.undo_stack_len(),
        frame.redo_stack_len(),
    );
    let draft = input.board_picker.appearance.as_mut().unwrap();
    draft.kind = BoardGridKind::IsometricDots;
    draft.kind_dirty = true;
    // Another operation changes an untouched field while the draft is open.
    input.boards.active_board_mut().spec.grid = BoardGrid::new(BoardGridKind::None, 63);
    assert!(input.apply_board_appearance());
    assert_eq!(
        input.boards.active_board().spec.grid,
        BoardGrid::new(BoardGridKind::IsometricDots, 63)
    );
    assert_eq!(input.color_for_tool(crate::input::Tool::Pen), pen);
    assert!(input.is_session_dirty());
    assert!(input.boards.active_board().appearance_explicit);
    let frame = input.boards.active_frame();
    assert_eq!(
        (
            frame.shapes.len(),
            frame.undo_stack_len(),
            frame.redo_stack_len()
        ),
        history
    );
}

#[test]
fn appearance_invalid_spacing_conflict_and_identity_change_keep_draft_unapplied() {
    let mut input = editing();
    let draft = input.board_picker.appearance.as_mut().unwrap();
    draft.spacing = "201".into();
    draft.spacing_dirty = true;
    assert!(!input.apply_board_appearance());
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "201");
    assert!(!input.is_session_dirty());
    input.board_picker.appearance.as_mut().unwrap().spacing = "20".into();
    input.board_appearance_palette(RED);
    input.boards.active_board_mut().spec.background = BoardBackground::Solid(BLUE);
    assert!(!input.apply_board_appearance());
    assert_eq!(
        input.boards.active_board().spec.background,
        BoardBackground::Solid(BLUE)
    );
    input.boards.bump_board_identity_generation();
    assert!(!input.apply_board_appearance());
    assert!(!input.is_session_dirty());
}

#[test]
fn appearance_keyboard_edits_spacing_and_pattern_and_selection_cancels() {
    let mut input = editing();
    input.board_appearance_key(Key::Tab);
    input.board_appearance_key(Key::Right);
    input.board_appearance_key(Key::Tab);
    input.board_appearance_key(Key::Backspace);
    input.board_appearance_key(Key::Backspace);
    input.board_appearance_key(Key::Char('2'));
    input.board_appearance_key(Key::Char('0'));
    assert!(input.apply_board_appearance());
    assert_eq!(
        input.boards.active_board().spec.grid,
        BoardGrid::new(BoardGridKind::Cartesian, 20)
    );
    input.board_picker_edit_color_selected_with_measurer(&crate::draw::TextMeasurer::default());
    input.board_picker_set_selected(0);
    assert!(input.board_appearance_edit().is_none());
}
