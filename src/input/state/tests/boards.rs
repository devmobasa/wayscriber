use super::*;
use crate::draw::ShapeId;
use crate::input::state::core::board_picker::BoardPickerState;
use crate::input::{BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardManager};

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn assert_board_text(state: &InputState, board_id: &str, shape_id: ShapeId, expected: &str) {
    let index = board_index(state, board_id);
    let shape = state.boards.board_states()[index]
        .pages
        .active_frame()
        .shape(shape_id)
        .expect("text shape");
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, expected),
        _ => panic!("Expected text shape"),
    }
}

fn disable_board_auto_create(state: &mut InputState) {
    let mut config = state.boards.to_config();
    config.auto_create = false;
    state.boards = BoardManager::from_config(config);
}

#[test]
fn switch_board_force_does_not_toggle_back_to_transparent() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);

    state.switch_board_force(BOARD_ID_WHITEBOARD);
    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
}

#[test]
fn switch_board_recent_skips_current_and_missing_entries() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.board_recent = vec![
        BOARD_ID_WHITEBOARD.to_string(),
        "missing".to_string(),
        "blackboard".to_string(),
    ];

    state.switch_board_recent();

    assert_eq!(state.board_id(), "blackboard");
}

#[test]
fn switch_board_recent_shows_toast_when_no_other_recent_board_exists() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    state.board_recent = vec![BOARD_ID_WHITEBOARD.to_string(), "missing".to_string()];

    state.switch_board_recent();

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("No recent board to switch to.")
    );
}

#[test]
fn switch_board_updates_open_board_picker_selection_and_clears_hover() {
    let mut state = create_test_input_state();
    state.open_board_picker();

    if let BoardPickerState::Open { hover_index, .. } = &mut state.board_picker_state {
        *hover_index = Some(0);
    }

    state.switch_board("blackboard");

    assert_eq!(state.board_id(), "blackboard");
    assert_eq!(
        state.board_picker_selected_index(),
        state.board_picker_row_for_board(state.boards.active_index())
    );
    match &state.board_picker_state {
        BoardPickerState::Open { hover_index, .. } => assert!(hover_index.is_none()),
        BoardPickerState::Hidden => panic!("board picker should remain open"),
    }
}

#[test]
fn switch_board_cancels_active_drawing_through_lifecycle_transition() {
    let mut state = create_test_input_state();
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![1.0, 1.0],
    };
    state.begin_pointer_drag(MouseButton::Left, None);
    state.needs_redraw = false;

    state.switch_board(BOARD_ID_WHITEBOARD);

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.active_drag_button.is_none());
    assert!(state.needs_redraw);
}

#[test]
fn failed_switch_board_preserves_active_interaction() {
    let mut state = create_test_input_state();
    disable_board_auto_create(&mut state);
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (30, 40)],
        point_thicknesses: vec![1.0, 1.0],
    };
    state.begin_pointer_drag(MouseButton::Left, None);
    state.needs_redraw = false;

    state.switch_board("missing-board");

    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);
    assert!(matches!(state.state, DrawingState::Drawing { .. }));
    assert_eq!(state.active_drag_button, Some(MouseButton::Left));
    assert!(!state.needs_redraw);
}

#[test]
fn switch_board_cancels_text_edit_on_source_board_before_switching() {
    let mut state = create_test_input_state();
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: "Original".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_board_text(&state, BOARD_ID_TRANSPARENT, shape_id, "");

    state.switch_board(BOARD_ID_WHITEBOARD);

    assert_eq!(state.board_id(), BOARD_ID_WHITEBOARD);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());
    assert_board_text(&state, BOARD_ID_TRANSPARENT, shape_id, "Original");
}

#[test]
fn duplicate_board_from_transparent_shows_info_toast_without_creating_board() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();
    assert_eq!(state.board_id(), BOARD_ID_TRANSPARENT);

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), initial_count);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Overlay board cannot be duplicated.")
    );
}

#[test]
fn duplicate_board_cancels_text_input_through_lifecycle_transition() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let initial_count = state.boards.board_count();
    state.text_wrap_width = Some(240);
    state.state = DrawingState::TextInput {
        x: 10,
        y: 20,
        buffer: "draft".to_string(),
    };
    state.needs_redraw = false;

    state.duplicate_board();

    assert_eq!(state.boards.board_count(), initial_count + 1);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_wrap_width.is_none());
    assert!(state.needs_redraw);
}

#[test]
fn duplicate_board_cancels_text_edit_before_cloning_board() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    let shape_id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 40,
        y: 80,
        text: "Original".to_string(),
        color: state.current_color,
        size: state.current_font_size,
        font_descriptor: state.font_descriptor.clone(),
        background_enabled: state.text_background_enabled,
        wrap_width: None,
    });
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());
    assert_board_text(&state, BOARD_ID_WHITEBOARD, shape_id, "");

    state.duplicate_board();

    assert!(matches!(state.state, DrawingState::Idle));
    assert!(state.text_edit_target.is_none());
    assert_board_text(&state, BOARD_ID_WHITEBOARD, shape_id, "Original");

    let duplicated_id = state.board_id().to_string();
    assert_ne!(duplicated_id, BOARD_ID_WHITEBOARD);
    assert_board_text(&state, &duplicated_id, shape_id, "Original");
}

#[test]
fn create_board_adds_board_queues_config_save_and_emits_toast() {
    let mut state = create_test_input_state();
    let initial_count = state.boards.board_count();

    assert!(state.create_board());

    assert_eq!(state.boards.board_count(), initial_count + 1);
    assert!(state.take_pending_board_config().is_some());
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.starts_with("Board created:"))
    );
}
