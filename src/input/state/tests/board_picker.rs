use super::create_test_input_state;

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
