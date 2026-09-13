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
    // Tab selects the size, so typing the original value replaces it.
    input.board_appearance_key(Key::Tab);
    input.board_appearance_key(Key::Char('4'));
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
    let (x, y, width, height) = input.board_appearance_frame().unwrap().bounds();
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
            f64::from(rect.x) <= x
                && f64::from(rect.y) <= y
                && f64::from(rect.x + rect.width) >= x + width
                && f64::from(rect.y + rect.height) >= y + height
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
    let mut input = laid_out(1920, 1080);
    input.board_picker.appearance.as_mut().unwrap().kind = BoardGridKind::IsometricDots;
    input.board_picker.appearance.as_mut().unwrap().spacing = "63".into();
    let original = BoardAppearance::from_spec(&input.boards.active_board().spec);
    let layout = input.board_picker_layout().unwrap();
    let x = (layout.origin_x + layout.padding_x + 1.0) as i32;
    let y = (layout.palette_top + 1.0) as i32;
    let expected_color = input.board_picker_palette_color_at(x, y).unwrap();
    let (left, top, width, height) = input.board_appearance_frame().unwrap().bounds();
    assert!(
        f64::from(x) < left
            || f64::from(x) > left + width
            || f64::from(y) < top
            || f64::from(y) > top + height
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
fn size_slider_uses_a_log_scale_that_round_trips_every_size() {
    assert_eq!(slider_to_size(0.0), 8);
    assert_eq!(slider_to_size(1.0), 200);
    assert!((size_to_slider(40) - 0.5).abs() < 1e-9);
    for size in 8..=200 {
        assert_eq!(slider_to_size(size_to_slider(size)), size);
    }
}

#[test]
fn dragging_the_size_slider_updates_the_draft_until_release() {
    let mut input = laid_out(900, 700);
    let row = input.board_appearance_size_row().unwrap();
    let track = input
        .board_appearance_frame()
        .unwrap()
        .to_surface(row.track);
    let (track_x, track_y, track_width, track_height) = track;
    let middle_y = (track_y + track_height / 2.0) as i32;

    assert!(input.board_appearance_press(track_x as i32 + 1, middle_y));
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "8");
    assert!(input.board_appearance_drag_to((track_x + track_width) as i32, middle_y));
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "200");

    // Releasing outside the sheet only ends the drag.
    assert!(input.board_appearance_click(1, 1));
    let draft = input.board_appearance_edit().unwrap();
    assert!(!draft.size_dragging);
    assert_eq!(draft.spacing, "200");
    assert!(!input.board_appearance_drag_to(track_x as i32, middle_y));
    assert!(!input.is_session_dirty());
}

#[test]
fn size_field_replaces_typed_values_and_arrows_step_the_size() {
    let mut input = laid_out(900, 700);
    let row = input.board_appearance_size_row().unwrap();
    let (x, y) = center(&input, row.field);
    assert!(input.board_appearance_click(x, y));
    assert_eq!(
        input.board_appearance_edit().unwrap().focus,
        AppearanceField::Spacing
    );

    for ch in ['2', '4', 'x'] {
        input.board_appearance_key(Key::Char(ch));
    }
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "24");

    input.board_appearance_key(Key::Up);
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "25");
    input.modifiers.shift = true;
    input.board_appearance_key(Key::Down);
    input.modifiers.shift = false;
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "15");
}

#[test]
fn wheel_steps_the_size_row_and_the_sheet_swallows_other_scrolling() {
    let mut input = laid_out(900, 700);
    let row = input.board_appearance_size_row().unwrap();
    let (x, y) = center(&input, row.field);

    assert!(input.board_appearance_wheel(x, y, -1));
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "41");
    assert!(input.board_appearance_wheel(x, y, 1));
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "40");

    // Over the preview: inside the sheet but outside the size row.
    let width = input.board_appearance_frame().unwrap().width;
    let (x, y) = center(&input, (0.0, 120.0, width, 0.0));
    assert!(input.board_appearance_wheel(x, y, 1));
    assert_eq!(input.board_appearance_edit().unwrap().spacing, "40");
    assert!(!input.board_appearance_wheel(1, 1, 1));
}

#[test]
fn cancel_sits_before_apply_and_both_buttons_work() {
    let mut input = laid_out(900, 700);
    let buttons = input.board_appearance_buttons().unwrap();
    assert!(buttons.cancel.0 + buttons.cancel.2 <= buttons.apply.0);

    input.board_picker.appearance.as_mut().unwrap().kind = BoardGridKind::Cartesian;
    let (x, y) = center(&input, buttons.cancel);
    assert!(input.board_appearance_click(x, y));
    assert!(input.board_appearance_edit().is_none());
    assert_eq!(
        input.boards.active_board().spec.grid.kind,
        BoardGridKind::None
    );

    input.board_picker_edit_color_selected_with_measurer(&crate::draw::TextMeasurer::default());
    input.board_picker.appearance.as_mut().unwrap().kind = BoardGridKind::Cartesian;
    let (x, y) = center(&input, buttons.apply);
    assert!(input.board_appearance_click(x, y));
    assert!(input.board_appearance_edit().is_none());
    assert_eq!(
        input.boards.active_board().spec.grid.kind,
        BoardGridKind::Cartesian
    );
}

fn laid_out(width: i32, height: i32) -> InputState {
    let mut input = editing();
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    input.update_board_picker_layout(&ctx, width as u32, height as u32);
    input
}

/// The surface pixel at the center of a rectangle in sheet units.
fn center(input: &InputState, rect: (f64, f64, f64, f64)) -> (i32, i32) {
    let (x, y, width, height) = input.board_appearance_frame().unwrap().to_surface(rect);
    ((x + width / 2.0) as i32, (y + height / 2.0) as i32)
}

#[test]
fn sheet_doubles_where_the_output_has_room_and_shrinks_on_small_outputs() {
    for (width, height, scale) in [
        (1920, 1080, 2.0),
        (1280, 720, 2.0),
        (900, 700, 2.0),
        (640, 480, 1.25),
        (420, 300, 1.0),
    ] {
        let input = laid_out(width, height);
        let frame = input.board_appearance_frame().unwrap();
        let (left, top, sheet_width, sheet_height) = frame.bounds();

        assert_eq!(frame.scale, scale, "{width}x{height}");
        assert!(
            left >= 0.0 && left + sheet_width <= f64::from(width),
            "{width}x{height}: {frame:?}"
        );
        assert!(
            top >= 0.0 && top + sheet_height <= f64::from(height),
            "{width}x{height}: {frame:?}"
        );
    }
}

#[test]
fn doubled_sheet_leans_toward_the_page_column_inside_the_picker() {
    let input = laid_out(1920, 1080);
    let layout = *input.board_picker_layout().unwrap();
    let (left, _, width, _) = input.board_appearance_frame().unwrap().bounds();
    let picker_right = layout.origin_x + layout.width;

    assert!(
        left >= layout.origin_x - 0.5 && left + width <= picker_right + 0.5,
        "sheet {left}+{width} outside picker {}..{picker_right}",
        layout.origin_x
    );
    assert!(left + width / 2.0 > layout.origin_x + layout.width / 2.0);
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

    let (x, y) = center(&input, header.color_field);
    assert!(input.board_appearance_click(x, y));
    assert_eq!(
        input.board_appearance_edit().unwrap().focus,
        AppearanceField::Color
    );

    input.board_picker.appearance.as_mut().unwrap().kind = BoardGridKind::Cartesian;
    let (x, y) = center(&input, header.close);
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

#[test]
fn command_palette_edit_board_paper_opens_the_sheet_on_the_active_board() {
    use crate::domain::Action;

    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut input = TestInputStateBuilder::default().build();
    input.switch_board_force("whiteboard");
    input.update_screen_dimensions(1280, 720);
    input.toggle_command_palette();

    input.command_palette.set_query("grid");
    assert!(
        input
            .filtered_commands()
            .iter()
            .any(|command| command.action == Action::BoardPaperEdit)
    );
    input.command_palette.set_query("paper");
    assert_eq!(
        input
            .filtered_commands()
            .first()
            .map(|command| command.action),
        Some(Action::BoardPaperEdit)
    );

    assert!(input.handle_command_palette_key_with_resources(resources, Key::Return));
    assert!(!input.command_palette.is_open());
    assert!(input.is_board_picker_open());
    assert!(!input.board_picker_is_quick());
    assert_eq!(
        input
            .board_appearance_edit()
            .map(BoardAppearanceEdit::board_id),
        Some("whiteboard")
    );
}

/// The sheet's hex field opens the full color picker on the draft, with the
/// picker and its sheet left open underneath.
fn open_paper_picker(input: &mut InputState) {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1280, 720).unwrap();
    let ctx = cairo::Context::new(&surface).unwrap();
    input.update_board_picker_layout(&ctx, 1280, 720);
    let frame = input.board_appearance_frame().expect("sheet frame");
    let header = input.board_appearance_header().expect("sheet header");
    let (fx, fy, fw, fh) = frame.to_surface(header.color_field);
    let (x, y) = ((fx + fw / 2.0) as i32, (fy + fh / 2.0) as i32);
    assert!(input.board_appearance_click(x, y));
}

#[test]
fn clicking_the_color_field_opens_the_picker_on_the_draft_and_keeps_the_sheet() {
    let mut input = editing();
    let draft = input.board_appearance_edit().unwrap().color.clone();

    open_paper_picker(&mut input);

    assert!(input.is_color_picker_popup_open());
    assert!(input.color_picker_popup_edits_board_paper());
    assert!(input.is_board_picker_open());
    assert!(input.board_appearance_edit().is_some());
    assert_eq!(
        input.color_picker_popup_current_color().map(color_to_hex),
        Some(draft),
        "the popup starts on the draft color"
    );
    assert_eq!(input.color_picker_popup_title(), "Paper Color");
    assert!(!input.color_picker_popup_shows_default_button());
    input.update_color_picker_popup_layout(1280, 720);
    let layout = input.color_picker_popup_layout().unwrap();
    assert_eq!(layout.alpha_h, 0.0, "paper has no alpha bar");
    // Sampling from the screen would close the sheet and land on the pen, so
    // the paper popup has no eyedropper.
    assert!(!layout.eyedropper_enabled);
    let (bx, by) = (
        layout.eyedropper_btn_x + layout.action_btn_size / 2.0,
        layout.eyedropper_btn_y + layout.action_btn_size / 2.0,
    );
    assert_eq!(layout.action_at(bx, by), None);
    assert_eq!(layout.action_tooltip_at(bx, by), None);
}

#[test]
fn the_wheel_does_not_reach_the_size_control_under_the_paper_picker() {
    let mut input = editing();
    open_paper_picker(&mut input);
    let frame = input.board_appearance_frame().unwrap();
    let row = input.board_appearance_size_row().unwrap();
    let (rx, ry, rw, rh) = frame.to_surface(row.track);
    let spacing = input.board_appearance_edit().unwrap().spacing.clone();

    assert!(!input.board_appearance_wheel((rx + rw / 2.0) as i32, (ry + rh / 2.0) as i32, 1));
    assert!(input.modal_owns_wheel(), "the registry swallows the tick");
    assert_eq!(input.board_appearance_edit().unwrap().spacing, spacing);
}

#[test]
fn picker_edits_preview_on_the_draft_and_cancel_restores_it() {
    let mut input = editing();
    let original = input.board_appearance_edit().unwrap().color.clone();
    let pen = input.color_for_tool(crate::input::Tool::Pen);
    open_paper_picker(&mut input);

    input.color_picker_popup_set_color(RED);
    assert_eq!(
        input.board_appearance_edit().unwrap().color,
        color_to_hex(RED)
    );
    assert_eq!(input.board_appearance_edit().unwrap().preview().0, RED);
    // Nothing behind the sheet moves until the sheet's own Apply.
    assert_ne!(
        input.boards.active_board().spec.background,
        BoardBackground::Solid(RED)
    );
    assert_eq!(input.color_for_tool(crate::input::Tool::Pen), pen);
    assert!(!input.is_session_dirty());

    input.close_color_picker_popup(true);
    assert!(!input.is_color_picker_popup_open());
    assert!(input.board_appearance_edit().is_some(), "the sheet stays");
    assert_eq!(input.board_appearance_edit().unwrap().color, original);
}

#[test]
fn picker_ok_keeps_the_draft_color_and_only_apply_writes_the_board() {
    let mut input = editing();
    let pen = input.color_for_tool(crate::input::Tool::Pen);
    let recents = input.recent_colors().len();
    open_paper_picker(&mut input);

    input.color_picker_popup_set_color(BLUE);
    input.apply_color_picker_popup();

    assert!(!input.is_color_picker_popup_open());
    assert_eq!(
        input.board_appearance_edit().unwrap().color,
        color_to_hex(BLUE)
    );
    assert_eq!(input.color_for_tool(crate::input::Tool::Pen), pen);
    assert_eq!(
        input.recent_colors().len(),
        recents,
        "paper is not a pen color"
    );
    assert!(!input.is_session_dirty());

    assert!(input.apply_board_appearance());
    assert_eq!(
        input.boards.active_board().spec.background,
        BoardBackground::Solid(BLUE)
    );
    assert!(input.is_session_dirty());
}

#[test]
fn ok_with_the_opening_color_typed_back_undoes_an_earlier_preview() {
    let mut input = editing();
    // A color with a three-digit spelling, so it is only parsed on OK.
    assert!(input.board_appearance_palette(crate::draw::WHITE));
    open_paper_picker(&mut input);
    input.color_picker_popup_set_color(RED);
    assert_eq!(
        input.board_appearance_edit().unwrap().color,
        color_to_hex(RED)
    );

    input.color_picker_popup_set_hex_editing(true);
    for ch in "#FFF".chars() {
        input.color_picker_popup_hex_append(ch);
    }
    input.apply_color_picker_popup();

    assert!(!input.is_color_picker_popup_open());
    assert_eq!(input.board_appearance_edit().unwrap().color, "#FFFFFF");
    assert!(!input.is_session_dirty());
}

#[test]
fn picker_colors_for_paper_are_always_opaque() {
    let mut input = editing();
    open_paper_picker(&mut input);

    input.color_picker_popup_set_alpha(0.25);
    assert_eq!(input.color_picker_popup_alpha(), Some(1.0));
    input.color_picker_popup_set_color(Color { a: 0.5, ..RED });
    assert_eq!(input.color_picker_popup_current_color(), Some(RED));
    assert_eq!(input.board_appearance_edit().unwrap().color, "#FF0000");

    // Typed hex is another way in: an alpha pair previews, commits, and
    // copies as opaque, so the popup never shows a color the paper cannot be.
    input.color_picker_popup_set_hex_editing(true);
    for ch in "#0000FF80".chars() {
        input.color_picker_popup_hex_append(ch);
    }
    assert_eq!(input.color_picker_popup_current_color(), Some(BLUE));
    assert!(input.color_picker_popup_commit_hex());
    assert_eq!(input.color_picker_popup_current_color(), Some(BLUE));
    assert_eq!(input.color_picker_popup_hex_buffer(), Some("#0000FF"));
    assert_eq!(input.board_appearance_edit().unwrap().color, "#0000FF");

    // And through OK with an uncommitted buffer.
    input.color_picker_popup_set_hex_editing(true);
    for ch in "#00FF0080".chars() {
        input.color_picker_popup_hex_append(ch);
    }
    input.apply_color_picker_popup();
    assert_eq!(input.board_appearance_edit().unwrap().color, "#00FF00");
}

#[test]
fn closing_the_sheet_or_picker_takes_the_paper_picker_with_it() {
    let mut input = editing();
    open_paper_picker(&mut input);
    input.board_picker_cancel_edit();
    assert!(!input.is_color_picker_popup_open());
    assert!(input.is_board_picker_open());

    input.board_picker_edit_color_selected_with_measurer(&crate::draw::TextMeasurer::default());
    open_paper_picker(&mut input);
    input.close_board_picker();
    assert!(!input.is_color_picker_popup_open());
    assert!(input.board_appearance_edit().is_none());
}

#[test]
fn space_on_the_color_field_opens_the_picker_and_escape_closes_only_the_picker() {
    let mut input = editing();
    assert_eq!(
        input.board_appearance_edit().unwrap().focus,
        AppearanceField::Color
    );
    assert!(input.board_appearance_key(Key::Space));
    assert!(input.color_picker_popup_edits_board_paper());
    // A typed space is not the shortcut; it reaches the field like other text.
    input.close_color_picker_popup(true);
    assert!(input.board_appearance_key(Key::Char(' ')));
    assert!(!input.is_color_picker_popup_open());
    assert!(input.board_appearance_key(Key::Space));

    // The picker has key precedence over the board picker.
    assert!(input.handle_color_picker_popup_key(Key::Escape));
    assert!(!input.is_color_picker_popup_open());
    assert!(input.board_appearance_edit().is_some());
}
