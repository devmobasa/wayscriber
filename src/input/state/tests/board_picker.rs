use super::create_test_input_state;
use crate::input::state::core::board_picker::BoardPickerDrag;

#[test]
fn board_picker_search_selects_transposed_match() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    for ch in "balckboard".chars() {
        input.board_picker_append_search(ch);
    }
    let selected = input.board_picker_selected_index().expect("selection");
    let name = &input.boards.board_states()[selected].spec.name;
    assert_eq!(name, "Blackboard");
}

#[test]
fn board_picker_search_selects_prefix_match() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    for ch in "blue".chars() {
        input.board_picker_append_search(ch);
    }
    let selected = input.board_picker_selected_index().expect("selection");
    let name = &input.boards.board_states()[selected].spec.name;
    assert_eq!(name, "Blueprint");
}

#[test]
fn board_picker_selects_recent_board() {
    let mut input = create_test_input_state();
    input.switch_board("blackboard");
    input.switch_board("whiteboard");
    input.open_board_picker_quick();
    let selected = input.board_picker_selected_index().expect("selection");
    let name = &input.boards.board_states()[selected].spec.name;
    assert_eq!(name, "Blackboard");
}

#[test]
fn board_picker_quick_mode_hides_new_row() {
    let mut input = create_test_input_state();
    let board_count = input.boards.board_count();
    input.open_board_picker_quick();
    assert_eq!(input.board_picker_row_count(), board_count);
    if board_count > 0 {
        assert!(!input.board_picker_is_new_row(board_count - 1));
    }
}

#[test]
fn board_picker_quick_mode_pins_board_to_top() {
    let mut input = create_test_input_state();
    let blackboard_index = input
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == "blackboard")
        .expect("blackboard board");
    input.switch_board("whiteboard");
    input.open_board_picker();
    input.board_picker_set_selected(blackboard_index);
    input.board_picker_toggle_pin_selected();
    input.open_board_picker_quick();
    input.board_picker_activate_row(0);
    assert_eq!(input.board_id(), "blackboard");
}

#[test]
fn board_picker_full_mode_pins_board_to_top() {
    let mut input = create_test_input_state();
    let blackboard_index = input
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == "blackboard")
        .expect("blackboard board");
    input.switch_board("whiteboard");
    input.open_board_picker();
    input.board_picker_set_selected(blackboard_index);
    input.board_picker_toggle_pin_selected();
    input.open_board_picker();
    input.board_picker_activate_row(0);
    assert_eq!(input.board_id(), "blackboard");
}

#[test]
fn board_picker_drag_pinned_clamped_to_pinned_section() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    let blackboard_index = input
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == "blackboard")
        .expect("blackboard board");
    let blackboard_row = input
        .board_picker_row_for_board(blackboard_index)
        .expect("row");
    input.board_picker_set_selected(blackboard_row);
    input.board_picker_toggle_pin_selected();
    let pinned_row = input
        .board_picker_row_for_board(blackboard_index)
        .expect("row");
    let last_row = input.boards.board_count().saturating_sub(1);
    input.board_picker_drag = Some(BoardPickerDrag {
        source_row: pinned_row,
        source_board: blackboard_index,
        current_row: last_row,
    });
    input.board_picker_finish_drag();
    let row_after = input
        .board_picker_row_for_board(blackboard_index)
        .expect("row");
    assert_eq!(row_after, pinned_row);
}

#[test]
fn board_picker_drag_unpinned_clamped_after_pinned() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    let blackboard_index = input
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == "blackboard")
        .expect("blackboard board");
    let blackboard_row = input
        .board_picker_row_for_board(blackboard_index)
        .expect("row");
    input.board_picker_set_selected(blackboard_row);
    input.board_picker_toggle_pin_selected();
    let whiteboard_index = input
        .boards
        .board_states()
        .iter()
        .position(|board| board.spec.id == "whiteboard")
        .expect("whiteboard board");
    let whiteboard_row = input
        .board_picker_row_for_board(whiteboard_index)
        .expect("row");
    input.board_picker_drag = Some(BoardPickerDrag {
        source_row: whiteboard_row,
        source_board: whiteboard_index,
        current_row: 0,
    });
    input.board_picker_finish_drag();
    let row_after = input
        .board_picker_row_for_board(whiteboard_index)
        .expect("row");
    assert!(row_after >= 1);
}
