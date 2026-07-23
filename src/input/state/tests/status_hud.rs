//! Status HUD hit-testing and press→release contract tests.
//!
//! Mirrors the board picker layout hit-tests and the toast activation
//! contract: the press side reports a hit without side effects, activation
//! happens on release-inside, and releases outside are ignored.

use super::*;
use crate::config::{StatusBarStyle, StatusPosition};
use crate::input::state::core::board_picker::BoardPickerFocus;
use crate::ui::StatusHudSegmentKind;

fn update_hud_layout(input: &mut InputState, width: u32, height: u32) {
    input.update_status_hud_layout(
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        width,
        height,
    );
}

fn segment_center(input: &InputState, kind: StatusHudSegmentKind) -> (i32, i32) {
    let layout = input.status_hud_layout().expect("status hud layout");
    let segment = layout
        .segments
        .iter()
        .find(|segment| segment.kind == kind)
        .unwrap_or_else(|| panic!("segment {kind:?} missing"));
    (
        (segment.x + segment.width / 2.0).round() as i32,
        (segment.y + segment.height / 2.0).round() as i32,
    )
}

#[test]
fn status_hud_layout_cleared_when_bar_hidden() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    assert!(input.status_hud_layout().is_some());

    input.show_status_bar = false;
    update_hud_layout(&mut input, 1280, 720);
    assert!(input.status_hud_layout().is_none());
    assert!(!input.status_hud_contains(20, 700));
}

#[test]
fn status_hud_hover_tracks_segments_and_requests_redraw() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Help);

    input.needs_redraw = false;
    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud_hover, Some(StatusHudSegmentKind::Help));
    assert!(input.needs_redraw, "hover transition requests a redraw");

    input.needs_redraw = false;
    input.zoom_chip_hover = Some(crate::ui::ZoomChipButtonKind::In);
    input.clear_chrome_hover();
    assert_eq!(input.status_hud_hover, None);
    assert_eq!(input.zoom_chip_hover, None);
    assert!(input.needs_redraw, "surface leave clears hover and redraws");

    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud_hover, Some(StatusHudSegmentKind::Help));

    // Moving off the pill clears hover (and redraws once more).
    input.needs_redraw = false;
    input.on_mouse_motion(5, 5);
    assert_eq!(input.status_hud_hover, None);
    assert!(input.needs_redraw);

    // A display-only HUD (`status_bar_interactive = false`) never hovers:
    // no affordance may advertise a click that would be rejected.
    input.status_bar_interactive = false;
    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud_hover, None);
}

#[test]
fn status_hud_reclassifies_hover_after_toolbar_hint_relayouts_segments() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Help);

    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud_hover, Some(StatusHudSegmentKind::Help));

    // Hiding the toolbar inserts its recovery segment before Help while the
    // physical pointer remains stationary.
    input.set_toolbar_visible(false);
    update_hud_layout(&mut input, 1280, 720);
    let segment_now_under_pointer = input
        .status_hud_layout()
        .and_then(|layout| layout.segment_at(x as f64, y as f64));
    assert_eq!(
        segment_now_under_pointer,
        Some(StatusHudSegmentKind::Toolbar)
    );
    assert_eq!(
        input.status_hud_hover, segment_now_under_pointer,
        "hover must follow rebuilt HUD geometry, not the old segment identity"
    );
}

#[test]
fn status_hud_layout_rebuild_preserves_cleared_hover_after_pointer_leave() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Help);
    input.on_mouse_motion(x, y);
    assert_eq!(input.status_hud_hover, Some(StatusHudSegmentKind::Help));

    // Pointer leave clears hover but retains the last coordinates. A redraw
    // may rebuild different geometry at those stale coordinates.
    input.clear_chrome_hover();
    input.set_toolbar_visible(false);
    input.update_status_hud_layout_for_pointer(
        StatusPosition::BottomLeft,
        &StatusBarStyle::default(),
        1280,
        720,
        false,
    );

    assert_eq!(
        input.status_hud_hover, None,
        "layout rebuild must not restore hover without main-surface pointer focus"
    );
}

#[test]
fn status_hud_press_reports_hit_without_side_effect() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Board);

    assert!(input.status_hud_contains(x, y));

    // The press side must not open anything or mutate interaction state.
    assert!(!input.is_board_picker_open());
    assert!(!input.is_color_picker_popup_open());
    assert!(!input.is_radial_menu_open());
    assert!(matches!(input.state, DrawingState::Idle));
}

#[test]
fn status_hud_click_board_segment_toggles_board_picker() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Board);

    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    assert_eq!(action, None);
    assert!(input.is_board_picker_open());
    // The Board chip lands on the board list, distinguishing it from the
    // Page chip which focuses the page panel.
    assert_eq!(input.board_picker_focus(), BoardPickerFocus::BoardList);
}

#[test]
fn status_hud_click_page_segment_opens_board_picker_page_panel() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Page);

    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    assert_eq!(action, None);
    // The page panel lives inside the board picker, and the Page chip
    // focuses it directly (unlike the Board chip's board list focus).
    assert!(input.is_board_picker_open());
    assert_eq!(input.board_picker_focus(), BoardPickerFocus::PagePanel);
}

#[test]
fn status_hud_click_color_dot_opens_color_picker_popup() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Color);

    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    assert_eq!(action, None);
    assert!(input.is_color_picker_popup_open());
}

#[test]
fn status_hud_click_tool_segment_opens_radial_menu() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Tool);

    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    assert_eq!(action, None);
    assert!(input.is_radial_menu_open());
}

#[test]
fn status_hud_click_help_segment_returns_toggle_help_action() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Help);

    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    assert_eq!(action, Some(Action::ToggleHelp));
    // The action is dispatched by the backend; no surface opens here.
    assert!(!input.is_board_picker_open());
    assert!(!input.is_color_picker_popup_open());
    assert!(!input.is_radial_menu_open());
}

#[test]
fn status_hud_click_toolbar_hint_returns_toggle_toolbar_action() {
    let mut input = create_test_input_state();

    // The hint chip only exists while every toolbar surface is hidden.
    update_hud_layout(&mut input, 1280, 720);
    let layout = input.status_hud_layout().expect("status hud layout");
    assert!(
        !layout
            .segments
            .iter()
            .any(|segment| segment.kind == StatusHudSegmentKind::Toolbar),
        "toolbar hint must not show while the toolbar is visible"
    );

    input.set_toolbar_visible(false);
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Toolbar);

    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    assert_eq!(action, Some(Action::ToggleToolbar));
    // The action is dispatched by the backend; no surface opens here.
    assert!(!input.is_board_picker_open());
    assert!(!input.is_color_picker_popup_open());
    assert!(!input.is_radial_menu_open());
}

/// End-to-end recovery in the shipping default state: pill side layout
/// (side palette retired) with the top strip F2-cycled to Hidden leaves
/// every raw visibility flag true while no surface is visible. The hint
/// chip must appear there, and dispatching its returned action must
/// actually restore the toolbar — the advertised recovery cannot be a
/// no-op.
#[test]
fn status_hud_toolbar_hint_recovers_cycle_hidden_strip_under_pill_layout() {
    let mut input = create_test_input_state();
    input.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Pill);
    input.handle_action(Action::CycleToolbarDisplay); // micro
    input.handle_action(Action::CycleToolbarDisplay); // hidden
    assert!(!input.toolbar_visible());

    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Toolbar);
    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    let action = action.expect("toolbar hint chip returns an action");
    assert_eq!(action, Action::ToggleToolbar);

    // Dispatch the returned action exactly as the backend does.
    input.handle_action(action);
    assert!(
        input.toolbar_visible(),
        "clicking the recovery chip must restore the toolbar"
    );

    // The hint disappears on the next layout pass.
    update_hud_layout(&mut input, 1280, 720);
    assert!(
        !input
            .status_hud_layout()
            .expect("status hud layout")
            .segments
            .iter()
            .any(|segment| segment.kind == StatusHudSegmentKind::Toolbar),
        "hint must clear once the toolbar is back"
    );
}

#[test]
fn status_hud_click_between_segments_consumes_without_action() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let layout = input.status_hud_layout().expect("layout");
    let first_segment_x = layout
        .segments
        .iter()
        .map(|segment| segment.x)
        .fold(f64::INFINITY, f64::min);
    let pill_x = layout.pill_x;
    let y = (layout.pill_y + layout.pill_height / 2.0).round() as i32;
    assert!(
        first_segment_x > pill_x + 2.0,
        "expected padding gap before the first segment"
    );
    let x = (pill_x + (first_segment_x - pill_x) / 2.0).round() as i32;

    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(hit);
    assert_eq!(action, None);
    assert!(!input.is_board_picker_open());
    assert!(!input.is_color_picker_popup_open());
    assert!(!input.is_radial_menu_open());
}

#[test]
fn status_hud_click_outside_is_ignored() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);

    let (hit, action) = input.check_status_hud_click(1279, 1);
    assert!(!hit);
    assert_eq!(action, None);
    assert!(!input.is_board_picker_open());
    assert!(!input.is_color_picker_popup_open());
    assert!(!input.is_radial_menu_open());
}

#[test]
fn status_hud_ignores_clicks_when_not_interactive() {
    let mut input = create_test_input_state();
    input.status_bar_interactive = false;
    update_hud_layout(&mut input, 1280, 720);
    // The HUD still renders identically: a layout exists...
    assert!(input.status_hud_layout().is_some());
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Tool);

    // ...but it consumes no clicks (pure display).
    assert!(!input.status_hud_contains(x, y));
    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(!hit);
    assert_eq!(action, None);
    assert!(!input.is_radial_menu_open());
}

#[test]
fn status_hud_press_routing_consumes_left_press_over_interactive_hud() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Tool);

    // A left press over the HUD must never start a stroke.
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(matches!(input.state, DrawingState::Idle));
    assert_eq!(input.boards.active_frame().shapes.len(), 0);

    // With interactivity disabled, presses draw through as before.
    input.status_bar_interactive = false;
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(matches!(input.state, DrawingState::Drawing { .. }));
}

#[test]
fn status_hud_ignored_while_radial_menu_overlays_it() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Tool);

    // Open the radial menu over the HUD (as the Tool chip does): its rings
    // render above the pill, so the HUD must stop consuming presses there.
    input.open_radial_menu(x as f64, y as f64);
    assert!(input.is_radial_menu_open());

    // The press side reports no hit while the menu is open.
    assert!(!input.status_hud_contains(x, y));

    // The release side cannot re-fire the chip either (no board picker or
    // second surface stacking over the open radial menu).
    let (hit, action) = input.check_status_hud_click(x, y);
    assert!(!hit);
    assert_eq!(action, None);
    assert!(!input.is_board_picker_open());

    // A routed press at the chip coordinate falls through to the radial
    // menu handlers instead of being swallowed by the HUD.
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(
        !input.status_hud_press_pending,
        "HUD must not claim presses under an open radial menu"
    );
}

#[test]
fn status_hud_ignored_while_other_eclipsing_overlays_are_open() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Page);
    assert!(input.status_hud_contains(x, y));

    // Board picker (also covers a picker opened between press and release:
    // check_status_hud_click shares the same guard).
    input.open_board_picker();
    assert!(!input.status_hud_contains(x, y));
    assert!(!input.check_status_hud_click(x, y).0);
    input.close_board_picker();
    assert!(input.status_hud_contains(x, y));

    // Color picker popup.
    input.open_color_picker_popup();
    assert!(!input.status_hud_contains(x, y));
    input.close_color_picker_popup(false);
    assert!(input.status_hud_contains(x, y));

    // Command palette and tour (belt-and-braces: the backend intercepts
    // these earlier for pointer/touch, but direct routing paths do not).
    input.command_palette_open = true;
    assert!(!input.status_hud_contains(x, y));
    input.command_palette_open = false;

    input.tour_active = true;
    assert!(!input.status_hud_contains(x, y));
    input.tour_active = false;
    assert!(input.status_hud_contains(x, y));
}

#[test]
fn tablet_path_press_release_activates_chip_via_routing() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Board);

    // Tablet input routes presses directly through the pointer chain (no
    // backend pending flag): the press consumes without opening anything...
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(matches!(input.state, DrawingState::Idle));
    assert!(!input.is_board_picker_open());
    assert!(input.status_hud_press_pending);

    // ...and the matching release inside the chip activates it.
    input.on_mouse_release_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(input.is_board_picker_open());
    assert!(!input.status_hud_press_pending);
}

#[test]
fn tablet_path_release_outside_hud_does_not_activate() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Board);

    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(input.status_hud_press_pending);

    // Releasing outside the pill consumes the pending press without
    // activating, and the flag does not leak into later releases.
    input.on_mouse_release_with_canvas(MouseButton::Left, 5, 5, 5, 5);
    assert!(!input.is_board_picker_open());
    assert!(!input.status_hud_press_pending);
    input.on_mouse_release_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(!input.is_board_picker_open());
}

#[test]
fn tablet_path_help_chip_dispatches_action_on_release() {
    let mut input = create_test_input_state();
    update_hud_layout(&mut input, 1280, 720);
    let (x, y) = segment_center(&input, StatusHudSegmentKind::Help);
    assert!(!input.show_help);

    // The help chip returns an action rather than opening a surface; on the
    // direct-routing path it dispatches through the shared action routing.
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(!input.show_help);
    input.on_mouse_release_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(input.show_help, "help overlay should toggle on release");
}
