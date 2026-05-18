use super::*;
use crate::draw::{BoardPages, Frame, ShapeId};
use crate::input::{BOARD_ID_BLACKBOARD, BOARD_ID_WHITEBOARD, BoardBackground};

fn board_index(state: &InputState, id: &str) -> usize {
    state
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == id)
        .expect("board index")
}

fn named_frame(name: &str) -> Frame {
    let mut frame = Frame::new();
    frame.set_page_name(Some(name.to_string()));
    frame
}

fn set_named_pages(state: &mut InputState, board_index: usize, names: &[&str], active: usize) {
    let pages = names.iter().map(|name| named_frame(name)).collect();
    state.boards.board_states_mut()[board_index].pages = BoardPages::from_pages(pages, active);
}

fn add_active_text_shape(state: &mut InputState, text: &str) -> ShapeId {
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

fn assert_page_text(
    state: &InputState,
    board_index: usize,
    page_index: usize,
    shape_id: ShapeId,
    expected: &str,
) {
    let shape = state.boards.board_states()[board_index].pages.pages()[page_index]
        .shape(shape_id)
        .expect("text shape");
    match &shape.shape {
        Shape::Text { text, .. } => assert_eq!(text, expected),
        _ => panic!("Expected text shape"),
    }
}

#[test]
fn set_board_name_trims_and_queues_config_save() {
    let mut state = create_test_input_state();
    let index = board_index(&state, BOARD_ID_BLACKBOARD);

    assert!(state.set_board_name(index, "  Focus Board  ".to_string()));

    assert_eq!(state.boards.board_states()[index].spec.name, "Focus Board");
    assert!(state.take_pending_board_config().is_some());
}

#[test]
fn set_board_name_rejects_empty_input_with_warning_toast() {
    let mut state = create_test_input_state();
    let index = board_index(&state, BOARD_ID_BLACKBOARD);
    let original = state.boards.board_states()[index].spec.name.clone();

    assert!(!state.set_board_name(index, "   \t ".to_string()));

    assert_eq!(state.boards.board_states()[index].spec.name, original);
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Board name cannot be empty.")
    );
}

#[test]
fn set_board_background_color_updates_active_auto_adjust_pen_color() {
    let mut state = create_test_input_state();
    let index = board_index(&state, BOARD_ID_WHITEBOARD);
    state.switch_board(BOARD_ID_WHITEBOARD);

    let new_color = Color {
        r: 0.1,
        g: 0.1,
        b: 0.1,
        a: 1.0,
    };

    assert!(state.set_board_background_color(index, new_color));

    let board = &state.boards.board_states()[index];
    assert!(matches!(board.spec.background, BoardBackground::Solid(color) if color == new_color));
    assert_eq!(
        board.spec.default_pen_color,
        Some(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        })
    );
    assert_eq!(
        state.current_color,
        board.spec.effective_pen_color().expect("pen color")
    );
    assert!(state.take_pending_board_config().is_some());
}

#[test]
fn reorder_page_in_board_moves_named_pages_and_active_index() {
    let mut state = create_test_input_state();
    let index = board_index(&state, BOARD_ID_BLACKBOARD);
    set_named_pages(&mut state, index, &["One", "Two", "Three"], 0);

    assert!(state.reorder_page_in_board(index, 0, 2));

    let pages = &state.boards.board_states()[index].pages;
    assert_eq!(pages.page_name(0), Some("Two"));
    assert_eq!(pages.page_name(1), Some("Three"));
    assert_eq!(pages.page_name(2), Some("One"));
    assert_eq!(pages.active_index(), 2);
}

#[test]
fn move_page_between_boards_copy_preserves_source_and_adds_page_to_target() {
    let mut state = create_test_input_state();
    let source = board_index(&state, BOARD_ID_WHITEBOARD);
    let target = board_index(&state, BOARD_ID_BLACKBOARD);
    set_named_pages(&mut state, source, &["Copied page"], 0);
    set_named_pages(&mut state, target, &["Target page"], 0);

    assert!(state.move_page_between_boards_with_activation(source, 0, target, true, false));

    assert_eq!(state.boards.board_states()[source].pages.page_count(), 1);
    assert_eq!(state.boards.board_states()[target].pages.page_count(), 2);
    assert_eq!(
        state.boards.board_states()[target].pages.page_name(1),
        Some("Copied page")
    );
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Page copied to 'Blackboard'"))
    );
}

#[test]
fn reset_active_canvas_position_clears_view_offset_on_solid_board() {
    let mut state = create_test_input_state();
    state.switch_board(BOARD_ID_WHITEBOARD);
    assert!(state.boards.active_frame_mut().set_view_offset(180, -90));

    assert!(state.reset_active_canvas_position());
    assert_eq!(state.boards.active_frame().view_offset(), (0, 0));
    assert_eq!(
        state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
        Some("Canvas position reset.")
    );
}

#[test]
fn move_page_between_boards_move_removes_source_page_and_activates_target_copy() {
    let mut state = create_test_input_state();
    let source = board_index(&state, BOARD_ID_WHITEBOARD);
    let target = board_index(&state, BOARD_ID_BLACKBOARD);
    set_named_pages(&mut state, source, &["Keep", "Move me"], 1);
    set_named_pages(&mut state, target, &["Target page"], 0);

    assert!(state.move_page_between_boards_with_activation(source, 1, target, false, false));

    assert_eq!(state.boards.board_states()[source].pages.page_count(), 1);
    assert_eq!(
        state.boards.board_states()[source].pages.page_name(0),
        Some("Keep")
    );
    assert_eq!(state.boards.board_states()[target].pages.page_count(), 2);
    assert_eq!(
        state.boards.board_states()[target].pages.page_name(1),
        Some("Move me")
    );
    assert_eq!(state.boards.board_states()[target].pages.active_index(), 1);
    assert!(
        state
            .ui_toast
            .as_ref()
            .is_some_and(|toast| toast.message.contains("Page moved to 'Blackboard'"))
    );
}

#[test]
fn switch_to_page_cancels_text_edit_before_leaving_source_page() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    set_named_pages(&mut state, board, &["Source", "Target"], 0);
    let shape_id = add_active_text_shape(&mut state, "Original");
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());

    assert!(state.switch_to_page(1));

    assert_eq!(state.boards.active_page_index(), 1);
    assert!(state.text_edit_target.is_none());
    assert_page_text(&state, board, 0, shape_id, "Original");
}

#[test]
fn page_duplicate_cancels_text_edit_before_cloning_source_page() {
    let mut state = create_test_input_state();
    let board = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_BLACKBOARD);
    let shape_id = add_active_text_shape(&mut state, "Original");
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());

    state.page_duplicate();

    assert_eq!(state.boards.active_page_index(), 1);
    assert!(state.text_edit_target.is_none());
    assert_page_text(&state, board, 0, shape_id, "Original");
    assert_page_text(&state, board, 1, shape_id, "Original");
}

#[test]
fn cross_board_page_copy_cancels_active_source_text_edit_before_cloning() {
    let mut state = create_test_input_state();
    let source = board_index(&state, BOARD_ID_WHITEBOARD);
    let target = board_index(&state, BOARD_ID_BLACKBOARD);
    state.switch_board(BOARD_ID_WHITEBOARD);
    set_named_pages(&mut state, source, &["Source"], 0);
    set_named_pages(&mut state, target, &["Target"], 0);
    let shape_id = add_active_text_shape(&mut state, "Original");
    state.set_selection(vec![shape_id]);
    assert!(state.edit_selected_text());

    assert!(state.move_page_between_boards_with_activation(source, 0, target, true, false));

    assert!(state.text_edit_target.is_none());
    assert_page_text(&state, source, 0, shape_id, "Original");
    assert_page_text(&state, target, 1, shape_id, "Original");
}
