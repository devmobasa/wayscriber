use super::*;
use crate::draw::{Frame, PageDeleteOutcome, ShapeId};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT};
use std::time::{Duration, Instant};

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn set_page_count(state: &mut InputState, board_index: usize, count: usize) {
    let pages = state.boards.board_states_mut()[board_index]
        .pages
        .pages_mut();
    pages.clear();
    pages.extend((0..count.max(1)).map(|_| Frame::new()));
}

fn add_text_shape(state: &mut InputState, text: &str) -> ShapeId {
    state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: text.to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    })
}

fn assert_active_text(state: &InputState, shape_id: ShapeId, expected: &str) {
    let shape = state
        .boards
        .active_frame()
        .shape(shape_id)
        .expect("text shape");
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, expected),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn delete_active_board_requires_confirmation_then_restore_recovers_board() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();
    state.switch_board(BOARD_ID_BLACKBOARD);

    state.delete_active_board();
    assert!(state.has_pending_board_delete());
    assert_eq!(state.boards.board_count(), initial_count);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Click to confirm."))
    );

    state.delete_active_board();
    assert!(!state.has_pending_board_delete());
    assert_eq!(state.boards.board_count(), initial_count - 1);
    assert_ne!(state.board_id(), BOARD_ID_BLACKBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board deleted: Blackboard")
    );

    state.restore_deleted_board();
    assert_eq!(state.boards.board_count(), initial_count);
    assert_eq!(state.board_id(), BOARD_ID_BLACKBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board restored: Blackboard")
    );
}

#[test]
fn delete_active_board_restore_preserves_cancelled_text_edit() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    let shape_id = add_text_shape(&mut state, "Original");
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_active_text(&state, shape_id, "");

    state.delete_active_board();
    state.delete_active_board();
    state.restore_deleted_board();

    assert_eq!(state.board_id(), BOARD_ID_BLACKBOARD);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());
    assert_active_text(&state, shape_id, "Original");
}

#[test]
fn restore_deleted_board_renames_reused_generated_id() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();
    assert!(state.create_board());
    let deleted_id = state.board_id().to_string();

    state.delete_active_board();
    state.delete_active_board();
    assert_eq!(state.boards.board_count(), initial_count);

    assert!(state.create_board());
    assert_eq!(state.board_id(), deleted_id);

    state.restore_deleted_board();

    assert_eq!(state.boards.board_count(), initial_count + 2);
    assert_ne!(state.board_id(), deleted_id);
    assert!(state.board_id().starts_with(&format!("{deleted_id}-")));

    let mut ids: Vec<_> = state
        .boards
        .board_states()
        .iter()
        .map(|board| board.spec.id.as_str())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), state.boards.board_count());
}

#[test]
fn cancel_pending_board_delete_clears_confirmation_state() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    state.delete_active_board();
    assert!(state.has_pending_board_delete());

    state.cancel_pending_board_delete();

    assert!(!state.has_pending_board_delete());
    assert_eq!(state.board_id(), BOARD_ID_BLACKBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board deletion cancelled.")
    );
}

#[test]
fn page_delete_requires_confirmation_and_restore_recovers_deleted_page() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);

    assert_eq!(state.page_delete(), PageDeleteOutcome::Pending);
    assert!(state.has_pending_page_delete());
    assert_eq!(state.boards.page_count(), 2);

    assert_eq!(state.page_delete(), PageDeleteOutcome::Removed);
    assert!(!state.has_pending_page_delete());
    assert_eq!(state.boards.page_count(), 1);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page deleted (1/1)")
    );

    state.restore_deleted_page();
    assert_eq!(state.boards.page_count(), 2);
    assert_eq!(state.boards.active_page_index(), 1);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page restored (2/2)")
    );
}

#[test]
fn page_delete_restore_preserves_cancelled_text_edit() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let shape_id = add_text_shape(&mut state, "Original");
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_active_text(&state, shape_id, "");

    assert_eq!(state.page_delete(), PageDeleteOutcome::Pending);
    assert_eq!(state.page_delete(), PageDeleteOutcome::Removed);
    state.restore_deleted_page();

    assert_eq!(state.boards.active_page_index(), 1);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());
    assert_active_text(&state, shape_id, "Original");
}

#[test]
fn cancel_pending_page_delete_clears_confirmation_state() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    state.page_delete();
    assert!(state.has_pending_page_delete());

    state.cancel_pending_page_delete();

    assert!(!state.has_pending_page_delete());
    assert_eq!(state.boards.page_count(), 2);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page deletion cancelled.")
    );
}

#[test]
fn page_delete_on_last_page_clears_shapes_without_removing_page() {
    let mut state = create_test_input_state();
    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });

    assert_eq!(state.page_delete(), PageDeleteOutcome::Cleared);

    assert_eq!(state.boards.page_count(), 1);
    assert!(state.boards.active_frame().shapes.is_empty());
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page cleared (last page)")
    );
}

#[test]
fn pending_board_delete_survives_active_drift_and_deletes_original_board() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    let requested_at = Instant::now();

    state.delete_active_board_at(requested_at);
    state.switch_board("whiteboard");
    state.delete_active_board_at(requested_at + Duration::from_millis(1));

    assert_eq!(state.board_id(), "whiteboard");
    assert!(!state.boards.has_board(BOARD_ID_BLACKBOARD));
    assert!(!state.has_pending_board_delete());
}

#[test]
fn board_rename_does_not_stale_pending_board_delete() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_BLACKBOARD);
    let requested_at = Instant::now();

    state.delete_active_board_at(requested_at);
    let index = board_index(&state, BOARD_ID_BLACKBOARD);
    assert!(state.set_board_name(index, "Renamed Board".to_string()));
    state.delete_active_board_at(requested_at + Duration::from_millis(1));

    assert!(!state.boards.has_board(BOARD_ID_BLACKBOARD));
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board deleted: Renamed Board")
    );
}

#[test]
fn page_content_edit_does_not_stale_pending_page_delete() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let requested_at = Instant::now();

    assert_eq!(state.page_delete(), PageDeleteOutcome::Pending);
    state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
        fill: false,
        color: state.current_color,
        thick: state.current_thickness,
    });
    assert_eq!(
        state.delete_active_page_at(requested_at + Duration::from_millis(1)),
        PageDeleteOutcome::Removed
    );

    assert_eq!(state.boards.page_count(), 1);
}

#[test]
fn pending_page_delete_survives_active_board_drift_and_deletes_original_page() {
    let mut state = create_test_input_state();
    let blackboard = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, blackboard, 2);
    let requested_at = Instant::now();

    assert_eq!(
        state.delete_active_page_at(requested_at),
        PageDeleteOutcome::Pending
    );
    state.switch_board(BOARD_ID_TRANSPARENT);
    assert_eq!(
        state.delete_active_page_at(requested_at + Duration::from_millis(1)),
        PageDeleteOutcome::Removed
    );

    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);
    assert_eq!(
        state.boards.board_states()[blackboard].pages.page_count(),
        1
    );
    assert!(!state.has_pending_page_delete());

    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![1.0, 1.0],
    };
    state.begin_pointer_drag(MouseButton::Left, None);
    state.restore_deleted_page();

    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);
    assert_eq!(
        state.boards.board_states()[blackboard].pages.page_count(),
        2
    );
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Page restored (2/2)")
    );
    assert!(matches!(state.state, DrawingState::Drawing { .. }));
    assert_eq!(state.active_drag_button, Some(MouseButton::Left));
}

#[test]
fn stale_active_page_delete_confirmation_does_not_cancel_active_interaction() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let requested_at = Instant::now();

    assert_eq!(
        state.delete_active_page_at(requested_at),
        PageDeleteOutcome::Pending
    );
    assert_eq!(
        state.boards.active_pages_mut().delete_page_at(1),
        PageDeleteOutcome::Removed
    );
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![1.0, 1.0],
    };
    state.begin_pointer_drag(MouseButton::Left, None);

    assert_eq!(
        state.delete_active_page_at(requested_at + Duration::from_millis(1)),
        PageDeleteOutcome::Pending
    );

    assert!(matches!(state.state, DrawingState::Drawing { .. }));
    assert_eq!(state.active_drag_button, Some(MouseButton::Left));
    assert!(!state.has_pending_page_delete());
}

#[test]
fn stale_board_panel_page_delete_confirmation_does_not_cancel_active_interaction() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_page_count(&mut state, board, 2);
    let requested_at = Instant::now();

    assert_eq!(
        state.delete_page_in_board_at(board, 0, requested_at),
        PageDeleteOutcome::Pending
    );
    assert_eq!(
        state.boards.board_states_mut()[board]
            .pages
            .delete_page_at(1),
        PageDeleteOutcome::Removed
    );
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![1.0, 1.0],
    };
    state.begin_pointer_drag(MouseButton::Left, None);

    assert_eq!(
        state.delete_page_in_board_at(board, 0, requested_at + Duration::from_millis(1)),
        PageDeleteOutcome::Pending
    );

    assert!(matches!(state.state, DrawingState::Drawing { .. }));
    assert_eq!(state.active_drag_button, Some(MouseButton::Left));
    assert!(!state.has_pending_page_delete());
}
