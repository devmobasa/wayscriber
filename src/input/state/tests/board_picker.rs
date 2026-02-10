use super::create_test_input_state;
use crate::draw::Frame;
use crate::input::BoardBackground;
use crate::input::state::core::board_picker::{BoardPickerDrag, BoardPickerFocus};
use crate::input::state::{
    PAGE_DELETE_ICON_MARGIN, PAGE_DELETE_ICON_SIZE, PAGE_NAME_HEIGHT, PAGE_NAME_PADDING,
};

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

fn update_picker_layout(state: &mut crate::input::state::InputState, width: i32, height: i32) {
    let surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).expect("image surface");
    let ctx = cairo::Context::new(&surface).expect("cairo context");
    state.update_board_picker_layout(&ctx, width as u32, height as u32);
}

fn set_board_page_count(
    state: &mut crate::input::state::InputState,
    board_index: usize,
    page_count: usize,
) {
    let pages = state.boards.board_states_mut()[board_index]
        .pages
        .pages_mut();
    pages.clear();
    pages.extend((0..page_count.max(1)).map(|_| Frame::new()));
}

#[test]
fn board_picker_page_hit_testing_uses_rendered_thumbnail_positions() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    update_picker_layout(&mut input, 1280, 720);

    let layout = *input.board_picker_layout().expect("layout");
    assert!(layout.page_panel_enabled);

    let thumb_x = layout.page_panel_x + 12.0;
    let thumb_y = layout.page_panel_y;

    assert_eq!(
        input.board_picker_page_index_at((thumb_x + 1.0) as i32, (thumb_y + 1.0) as i32),
        Some(0)
    );

    let icon_y =
        thumb_y + layout.page_thumb_height - PAGE_DELETE_ICON_SIZE * 0.5 - PAGE_DELETE_ICON_MARGIN;
    let rename_x = thumb_x + PAGE_DELETE_ICON_SIZE * 0.5 + PAGE_DELETE_ICON_MARGIN;
    let duplicate_x = thumb_x + layout.page_thumb_width * 0.5;
    let delete_x =
        thumb_x + layout.page_thumb_width - PAGE_DELETE_ICON_SIZE * 0.5 - PAGE_DELETE_ICON_MARGIN;

    assert_eq!(
        input.board_picker_page_rename_index_at(rename_x as i32, icon_y as i32),
        Some(0)
    );
    assert_eq!(
        input.board_picker_page_duplicate_index_at(duplicate_x as i32, icon_y as i32),
        Some(0)
    );
    assert_eq!(
        input.board_picker_page_delete_index_at(delete_x as i32, icon_y as i32),
        Some(0)
    );
}

#[test]
fn board_picker_empty_page_list_has_no_page_hit() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    let board_index = input
        .board_picker_page_panel_board_index()
        .expect("page panel board index");
    input.boards.board_states_mut()[board_index]
        .pages
        .pages_mut()
        .clear();

    update_picker_layout(&mut input, 1280, 720);

    let layout = *input.board_picker_layout().expect("layout");
    let add_x = (layout.page_panel_x + 12.0 + 1.0) as i32;
    let add_y = (layout.page_panel_y + 1.0) as i32;

    assert_eq!(input.board_picker_page_index_at(add_x, add_y), None);
    assert!(input.board_picker_page_add_card_at(add_x, add_y));
}

#[test]
fn board_picker_add_card_clickable_when_pages_exactly_fill_rows() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    update_picker_layout(&mut input, 1280, 720);

    let board_index = input
        .board_picker_page_panel_board_index()
        .expect("page panel board index");

    let mut layout = *input.board_picker_layout().expect("layout");
    let max_rows = layout.page_max_rows.max(1);
    if max_rows < 2 {
        return;
    }

    let mut target_pages = layout.page_cols.max(1) * (max_rows - 1);
    set_board_page_count(&mut input, board_index, target_pages);
    update_picker_layout(&mut input, 1280, 720);

    layout = *input.board_picker_layout().expect("layout");
    let cols = layout.page_cols.max(1);
    let expected_rows = target_pages.div_ceil(cols);
    if expected_rows >= max_rows {
        target_pages = cols * (max_rows - 1);
        set_board_page_count(&mut input, board_index, target_pages);
        update_picker_layout(&mut input, 1280, 720);
        layout = *input.board_picker_layout().expect("layout");
    }

    let row_stride =
        layout.page_thumb_height + PAGE_NAME_HEIGHT + PAGE_NAME_PADDING + layout.page_thumb_gap;
    let add_row = target_pages / layout.page_cols.max(1);
    assert!(add_row < max_rows);

    let add_x = (layout.page_panel_x + 12.0 + 1.0) as i32;
    let add_y = (layout.page_panel_y + add_row as f64 * row_stride + 1.0) as i32;

    assert!(input.board_picker_page_add_card_at(add_x, add_y));
    assert_eq!(input.board_picker_page_index_at(add_x, add_y), None);
}

#[test]
fn board_picker_overflow_hitbox_matches_rendered_hint_position() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    update_picker_layout(&mut input, 1280, 720);

    let board_index = input
        .board_picker_page_panel_board_index()
        .expect("page panel board index");

    let mut layout = *input.board_picker_layout().expect("layout");
    let mut overflow_pages = layout.page_cols.max(1) * layout.page_max_rows.max(1) + 1;
    set_board_page_count(&mut input, board_index, overflow_pages);
    update_picker_layout(&mut input, 1280, 720);

    layout = *input.board_picker_layout().expect("layout");
    let visible_capacity = layout.page_cols.max(1) * layout.page_max_rows.max(1);
    if overflow_pages <= visible_capacity {
        overflow_pages = visible_capacity + 1;
        set_board_page_count(&mut input, board_index, overflow_pages);
        update_picker_layout(&mut input, 1280, 720);
        layout = *input.board_picker_layout().expect("layout");
    }

    let hint_x = (layout.page_panel_x + 12.0 + 2.0) as i32;
    let hint_y =
        (layout.page_panel_y + layout.page_panel_height + layout.footer_font_size + 6.0) as i32;

    assert!(input.board_picker_page_overflow_at(hint_x, hint_y));
}

#[test]
fn board_picker_row_action_hitboxes_match_rendered_positions() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    update_picker_layout(&mut input, 1280, 720);

    let layout = *input.board_picker_layout().expect("layout");
    assert!(layout.handle_width > 0.0);
    assert!(layout.open_icon_size > 0.0);

    let row = 0usize;
    let row_center =
        layout.origin_y + layout.padding_y + layout.header_height + layout.row_height * 0.5;
    let list_right = layout.origin_x + layout.list_width;
    let handle_x = list_right - layout.padding_x - layout.handle_width + layout.handle_width * 0.5;
    let open_x = list_right
        - layout.padding_x
        - layout.handle_width
        - layout.open_icon_gap
        - layout.open_icon_size
        + layout.open_icon_size * 0.5;

    assert_eq!(
        input.board_picker_handle_index_at(handle_x as i32, row_center as i32),
        Some(row)
    );
    assert_eq!(
        input.board_picker_open_icon_index_at(open_x as i32, row_center as i32),
        Some(row)
    );
}

#[test]
fn board_picker_palette_hit_testing_uses_rendered_coordinates() {
    let mut input = create_test_input_state();
    input.open_board_picker();

    let solid_board_index = input
        .boards
        .board_states()
        .iter()
        .position(|board| matches!(board.spec.background, BoardBackground::Solid(_)))
        .expect("solid board");
    let solid_row = input
        .board_picker_row_for_board(solid_board_index)
        .expect("solid row");
    input.board_picker_set_selected(solid_row);
    input.board_picker_edit_color_selected();

    update_picker_layout(&mut input, 1280, 720);
    let layout = *input.board_picker_layout().expect("layout");
    assert!(layout.palette_rows > 0);
    assert!(layout.palette_cols > 0);

    let swatch_x = (layout.origin_x + layout.padding_x + 1.0) as i32;
    let swatch_y = (layout.palette_top + 1.0) as i32;
    assert!(
        input
            .board_picker_palette_color_at(swatch_x, swatch_y)
            .is_some()
    );
}

#[test]
fn board_picker_page_focus_clamps_to_existing_pages() {
    let mut input = create_test_input_state();
    input.open_board_picker();
    update_picker_layout(&mut input, 1280, 720);

    let board_index = input
        .board_picker_page_panel_board_index()
        .expect("page panel board index");
    set_board_page_count(&mut input, board_index, 1);
    update_picker_layout(&mut input, 1280, 720);

    let layout = *input.board_picker_layout().expect("layout");
    assert!(layout.page_panel_enabled);
    assert_eq!(layout.page_visible_count, 1);

    input.board_picker_set_focus(BoardPickerFocus::PagePanel);
    input.board_picker_set_page_focus_index(usize::MAX);
    assert_eq!(input.board_picker_page_focus_index(), Some(0));
}
