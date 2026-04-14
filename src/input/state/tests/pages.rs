use super::*;
use crate::draw::{BoardPages, Frame};
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

    assert!(state.move_page_between_boards(source, 0, target, true));

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

    assert!(state.move_page_between_boards(source, 1, target, false));

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
