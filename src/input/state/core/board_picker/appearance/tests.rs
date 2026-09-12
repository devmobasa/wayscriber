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
fn reverting_draft_fields_preserves_later_external_changes() {
    let mut input = editing();
    let original_color = input.board_appearance_edit().unwrap().color.clone();
    input.board_appearance_palette(RED);
    input.board_picker.appearance.as_mut().unwrap().color = original_color.to_lowercase();
    input.board_appearance_key(Key::Tab);
    input.board_appearance_key(Key::Right);
    input.board_appearance_key(Key::Left);
    input.board_appearance_key(Key::Tab);
    input.board_appearance_key(Key::Backspace);
    input.board_appearance_key(Key::Char('0'));

    let board = input.boards.active_board_mut();
    board.spec.background = BoardBackground::Solid(BLUE);
    board.spec.grid = BoardGrid::new(BoardGridKind::IsometricDots, 63);
    let current = BoardAppearance::from_spec(&board.spec);
    assert!(input.apply_board_appearance());
    assert_eq!(
        BoardAppearance::from_spec(&input.boards.active_board().spec),
        current
    );
    assert!(!input.is_session_dirty());
    assert!(!input.boards.active_board().appearance_explicit);
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

#[test]
fn appearance_preview_and_cancel_damage_the_sheet_without_changing_the_board() {
    let mut input = editing();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 900, 700).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    input.update_board_picker_layout(&ctx, 900, 700);
    let (x, y, width) = input.board_appearance_rect().unwrap();
    let original = BoardAppearance::from_spec(&input.boards.active_board().spec);
    input.dirty_tracker.take_regions(900, 700);
    for key in [Key::Tab, Key::Right, Key::Escape] {
        // Unrelated UI damage must not suppress the sheet through a nonempty clip.
        input
            .dirty_tracker
            .mark_rect(crate::util::Rect::new(0, 0, 10, 10).unwrap());
        assert!(input.board_appearance_key(key));
        let regions = input.dirty_tracker.take_regions(900, 700);
        assert!(regions.iter().any(|rect| {
            f64::from(rect.x) <= x - 12.0
                && f64::from(rect.y) <= y - 70.0
                && f64::from(rect.x + rect.width) >= x + width + 12.0
                && f64::from(rect.y + rect.height) >= y + 222.0
        }));
        assert_eq!(
            BoardAppearance::from_spec(&input.boards.active_board().spec),
            original
        );
        assert!(!input.is_session_dirty());
    }
    assert!(input.board_appearance_edit().is_none());
}

#[test]
fn visible_palette_click_keeps_pattern_and_spacing_draft_until_apply() {
    let mut input = editing();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 900, 700).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    input.update_board_picker_layout(&ctx, 900, 700);
    input.board_picker.appearance.as_mut().unwrap().kind = BoardGridKind::IsometricDots;
    input.board_picker.appearance.as_mut().unwrap().spacing = "63".into();
    let original = BoardAppearance::from_spec(&input.boards.active_board().spec);
    let layout = input.board_picker_layout().unwrap();
    let x = (layout.origin_x + layout.padding_x + 1.0) as i32;
    let y = (layout.palette_top + 1.0) as i32;
    let expected_color = input.board_picker_palette_color_at(x, y).unwrap();
    let (left, top, width) = input.board_appearance_rect().unwrap();
    assert!(
        f64::from(x) < left - 12.0
            || f64::from(x) >= left + width + 12.0
            || f64::from(y) < top - 70.0
            || f64::from(y) >= top + 222.0
    );
    assert!(input.board_appearance_click(x, y));
    let draft = input.board_appearance_edit().unwrap();
    assert_eq!(draft.kind, BoardGridKind::IsometricDots);
    assert_eq!(draft.spacing, "63");
    assert_eq!(draft.preview().0, expected_color);
    assert_eq!(
        BoardAppearance::from_spec(&input.boards.active_board().spec),
        original
    );
    assert!(!input.is_session_dirty());
    assert!(input.apply_board_appearance());
    assert_eq!(
        input.boards.active_board().spec.grid,
        BoardGrid::new(BoardGridKind::IsometricDots, 63)
    );
    assert_eq!(
        input.boards.active_board().spec.background,
        BoardBackground::Solid(expected_color)
    );
}

#[test]
fn spacing_presets_select_only_the_matching_value() {
    let mut input = editing();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 900, 700).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    input.update_board_picker_layout(&ctx, 900, 700);
    let (left, top, width) = input.board_appearance_rect().unwrap();
    let row = (top + 72.0) as i32;

    assert!(input.board_appearance_click((left + width - 60.0) as i32, row));
    let draft = input.board_appearance_edit().unwrap();
    assert_eq!(draft.spacing, "20");
    assert!(draft.spacing_matches(20));
    assert!(!draft.spacing_matches(40));

    assert!(input.board_appearance_click((left + width - 20.0) as i32, row));
    let draft = input.board_appearance_edit().unwrap();
    assert_eq!(draft.spacing, "40");
    assert!(draft.spacing_matches(40));
    assert!(!draft.spacing_matches(20));

    input.board_picker.appearance.as_mut().unwrap().spacing = "63".into();
    let draft = input.board_appearance_edit().unwrap();
    assert!(!draft.spacing_matches(20) && !draft.spacing_matches(40));
}

fn laid_out(width: i32, height: i32) -> InputState {
    let mut input = editing();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    input.update_board_picker_layout(&ctx, width as u32, height as u32);
    input
}

fn center((x, y, width, height): (f64, f64, f64, f64)) -> (i32, i32) {
    ((x + width / 2.0) as i32, (y + height / 2.0) as i32)
}

#[test]
fn sheet_floats_over_the_page_column_without_covering_the_board_list() {
    for (width, height) in [(1920, 1080), (1280, 720), (900, 700)] {
        let input = laid_out(width, height);
        let layout = *input.board_picker_layout().unwrap();
        let (x, _, sheet_width) = input.board_appearance_rect().unwrap();

        assert!(
            x - 12.0 >= layout.origin_x + layout.list_width,
            "{width}x{height}: the sheet covers the board list"
        );
        assert!(x + sheet_width + 12.0 <= layout.origin_x + layout.width);
    }
}

#[test]
fn sheet_header_close_cancels_and_color_field_takes_focus() {
    let mut input = laid_out(900, 700);
    let original = BoardAppearance::from_spec(&input.boards.active_board().spec);
    let header = input.board_appearance_header().unwrap();
    input.board_appearance_key(Key::Tab);
    assert_ne!(
        input.board_appearance_edit().unwrap().focus,
        AppearanceField::Color
    );

    let (x, y) = center(header.color_field);
    assert!(input.board_appearance_click(x, y));
    assert_eq!(
        input.board_appearance_edit().unwrap().focus,
        AppearanceField::Color
    );

    input.board_picker.appearance.as_mut().unwrap().kind = BoardGridKind::Cartesian;
    let (x, y) = center(header.close);
    assert!(input.board_appearance_click(x, y));
    assert!(input.board_appearance_edit().is_none());
    assert!(input.is_board_picker_open());
    assert_eq!(
        BoardAppearance::from_spec(&input.boards.active_board().spec),
        original
    );
    assert!(!input.is_session_dirty());
}

#[test]
fn opening_and_closing_the_sheet_damage_the_whole_surface() {
    let mut input = laid_out(900, 700);
    let full = |regions: &[crate::util::Rect]| {
        regions
            .iter()
            .any(|rect| rect.x <= 0 && rect.y <= 0 && rect.width >= 900 && rect.height >= 700)
    };
    input.dirty_tracker.take_regions(900, 700);

    input.board_appearance_key(Key::Escape);
    assert!(full(&input.dirty_tracker.take_regions(900, 700)));

    input.board_picker_edit_color_selected_with_measurer(&crate::draw::TextMeasurer::default());
    assert!(input.board_appearance_edit().is_some());
    assert!(full(&input.dirty_tracker.take_regions(900, 700)));
}
