//! Zoom chip hit-testing and press→release contract tests.
//!
//! Mirrors the status HUD tests: the press side reports a hit without side
//! effects, activation happens on release-inside, releases outside are
//! ignored, and visibility/interactivity are gated on the effective
//! `zoom_chip_enabled()` state.

use super::*;
use crate::config::StatusBarStyle;
use crate::ui::{ZoomChipButtonKind, ZoomChipPress};

fn update_chip_layout(input: &mut InputState, width: u32, height: u32) {
    input.update_zoom_chip_layout(&StatusBarStyle::default(), width, height);
}

fn button_center(input: &InputState, kind: ZoomChipButtonKind) -> (i32, i32) {
    let layout = input.zoom_chip_layout().expect("zoom chip layout");
    let button = layout
        .buttons
        .iter()
        .find(|button| button.kind == kind)
        .unwrap_or_else(|| panic!("button {kind:?} missing"));
    (
        (button.x + button.width / 2.0).round() as i32,
        (button.y + button.height / 2.0).round() as i32,
    )
}

/// Center of the passive `NN%` readout that sits between the ⊖ and ⊕ buttons:
/// inside the pill but off every actionable button.
fn passive_percent_center(input: &InputState) -> (i32, i32) {
    let layout = input.zoom_chip_layout().expect("zoom chip layout");
    let out = layout
        .buttons
        .iter()
        .find(|b| b.kind == ZoomChipButtonKind::Out)
        .expect("out button");
    let in_button = layout
        .buttons
        .iter()
        .find(|b| b.kind == ZoomChipButtonKind::In)
        .expect("in button");
    (
        ((out.x + out.width + in_button.x) / 2.0).round() as i32,
        (layout.pill_y + layout.pill_height / 2.0).round() as i32,
    )
}

#[test]
fn zoom_chip_layout_cleared_when_actions_hidden() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    assert!(input.zoom_chip_layout().is_some());

    // Visibility is gated on `show_zoom_actions`: off means no layout, no hit.
    input.show_zoom_actions = false;
    update_chip_layout(&mut input, 1280, 720);
    assert!(input.zoom_chip_layout().is_none());
    assert!(!input.zoom_chip_contains(1270, 710));
}

#[test]
fn toggle_zoom_chip_action_hides_layout_and_hit_testing() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    assert!(input.zoom_chip_layout().is_some());

    // The palette/keybinding toggle hides the chip without
    // touching the `show_zoom_actions` toolbar preference, and persists
    // its own preference across restarts.
    input.handle_action(crate::config::Action::ToggleZoomChip);
    assert!(!input.zoom_chip_enabled());
    assert!(input.show_zoom_actions, "toolbar preference untouched");
    assert_eq!(
        input.take_pending_backend_action(),
        Some(crate::input::state::PendingBackendAction::PersistZoomChipConfig(false))
    );
    update_chip_layout(&mut input, 1280, 720);
    assert!(input.zoom_chip_layout().is_none());
    assert!(!input.zoom_chip_contains(1270, 710));

    // Toggling again restores it.
    input.handle_action(crate::config::Action::ToggleZoomChip);
    assert!(input.zoom_chip_enabled());
    update_chip_layout(&mut input, 1280, 720);
    assert!(input.zoom_chip_layout().is_some());
}

#[test]
fn zoom_chip_hover_tracks_buttons_and_clears_when_hidden() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    input.needs_redraw = false;
    input.on_mouse_motion(x, y);
    assert_eq!(input.zoom_chip_hover, Some(ZoomChipButtonKind::In));
    assert!(input.needs_redraw, "hover transition requests a redraw");

    // Runtime-hiding the chip clears hover on the next layout pass.
    input.handle_action(crate::config::Action::ToggleZoomChip);
    update_chip_layout(&mut input, 1280, 720);
    assert_eq!(input.zoom_chip_hover, None);
}

#[test]
fn zoom_chip_reclassifies_hover_after_fit_removes_the_lock_button() {
    let mut input = create_test_input_state();
    input.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::Fit);

    input.on_mouse_motion_with_canvas(x, y, x, y);
    assert_eq!(input.zoom_chip_hover, Some(ZoomChipButtonKind::Fit));

    // Fit returns to 100%, removing Lock and shrinking the right-anchored
    // layout while the physical pointer remains stationary.
    input.set_zoom_status(false, false, 1.0, (0.0, 0.0));
    update_chip_layout(&mut input, 1280, 720);
    let button_now_under_pointer = input.zoom_chip_button_at(x, y);
    assert_ne!(button_now_under_pointer, Some(ZoomChipButtonKind::Fit));
    assert_eq!(
        input.zoom_chip_hover, button_now_under_pointer,
        "hover must follow the rebuilt geometry, not the old button identity"
    );
}

#[test]
fn zoom_chip_layout_rebuild_preserves_cleared_hover_after_pointer_leave() {
    let mut input = create_test_input_state();
    input.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);
    input.on_mouse_motion_with_canvas(x, y, x, y);
    assert_eq!(input.zoom_chip_hover, Some(ZoomChipButtonKind::In));

    // Pointer leave clears hover but retains the last coordinates. A redraw
    // must not reapply that stale hit while focus is elsewhere.
    input.clear_chrome_hover();
    input.update_zoom_chip_layout_for_pointer(&StatusBarStyle::default(), 1280, 720, false);

    assert_eq!(
        input.zoom_chip_hover, None,
        "layout rebuild must not restore hover without main-surface pointer focus"
    );
}

#[test]
fn while_zoomed_display_mode_shows_the_chip_only_during_zoom() {
    let mut input = create_test_input_state();
    input.zoom_chip_display = crate::config::ZoomChipDisplay::WhileZoomed;

    // At 100% the corner stays clean: no layout, no hit.
    update_chip_layout(&mut input, 1280, 720);
    assert!(!input.zoom_chip_enabled());
    assert!(input.zoom_chip_layout().is_none());

    // Zoom engages: the chip appears (and with it the one-indicator
    // handoff — zoom_chip_enabled() drives the fallback badges too).
    input.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    assert!(input.zoom_chip_enabled());
    update_chip_layout(&mut input, 1280, 720);
    assert!(input.zoom_chip_layout().is_some());

    // Zoom ends: clean corner again.
    input.set_zoom_status(false, false, 1.0, (0.0, 0.0));
    assert!(!input.zoom_chip_enabled());
    update_chip_layout(&mut input, 1280, 720);
    assert!(input.zoom_chip_layout().is_none());
}

#[test]
fn zoom_chip_press_reports_hit_without_side_effect() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    assert!(input.zoom_chip_contains(x, y));
    // A mere hit test never dispatches a zoom action or starts a stroke.
    assert!(matches!(input.state, DrawingState::Idle));
    assert_eq!(input.take_pending_zoom_action(), None);
}

#[test]
fn zoom_chip_click_out_returns_zoom_out() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::Out);

    // Press and release on the same button (⊖): the same-button contract fires.
    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::Out, x, y);
    assert!(hit);
    assert_eq!(action, Some(Action::ZoomOut));
}

#[test]
fn zoom_chip_click_in_returns_zoom_in() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::In, x, y);
    assert!(hit);
    assert_eq!(action, Some(Action::ZoomIn));
}

#[test]
fn zoom_chip_click_fit_returns_reset_zoom() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::Fit);

    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::Fit, x, y);
    assert!(hit);
    // "Fit" resets back to 100% — there is no separate fit action.
    assert_eq!(action, Some(Action::ResetZoom));
}

#[test]
fn zoom_chip_click_lock_returns_toggle_lock_when_zoomed() {
    let mut input = create_test_input_state();
    // The Lock toggle is present only while zoom is active.
    input.set_zoom_status(true, false, 2.0, (0.0, 0.0));
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::Lock);

    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::Lock, x, y);
    assert!(hit);
    assert_eq!(action, Some(Action::ToggleZoomLock));
}

#[test]
fn zoom_chip_activation_records_coach_slow_path() {
    // Activating a shortcut-bound zoom action from the chip feeds the shortcut
    // coach the same "you could have pressed the key" slow-path signal the
    // toolbar and command palette record. The chip dispatches through the
    // shared action path (handle_action) — the fast/keyboard path — so without
    // this seam the coach would never learn from chip use. Recorded at the
    // InputState level in check_zoom_chip_click.
    let mut input = create_test_input_state();
    assert!(
        input.shortcut_for_action(Action::ZoomIn).is_some(),
        "test relies on ZoomIn having a default shortcut"
    );
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::In, x, y);
    assert!(hit);
    assert_eq!(action, Some(Action::ZoomIn));
    assert_eq!(
        input.pending_onboarding_usage.shortcut_slow_path_action,
        Some(Action::ZoomIn),
        "zoom-chip activation must feed the coach slow path"
    );
    assert_eq!(input.pending_onboarding_usage.shortcut_slow_path_repeats, 1);
}

#[test]
fn zoom_chip_same_button_mismatch_does_not_coach() {
    // A release that lands off the pressed button fires no action, so there is
    // nothing to coach and no slow-path signal is recorded.
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    // Pressed ⊖ but released over ⊕: the same-button contract consumes the
    // release without an action.
    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::Out, x, y);
    assert!(hit);
    assert_eq!(action, None);
    assert_eq!(
        input.pending_onboarding_usage.shortcut_slow_path_action, None,
        "a no-op chip release must not build a coach streak"
    );
}

#[test]
fn zoom_chip_release_on_a_different_button_fires_nothing() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    // Pressed ⊖ but released over ⊕: the buttons differ, so the same-button
    // contract consumes the release (hit) without dispatching a zoom action.
    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::Out, x, y);
    assert!(hit);
    assert_eq!(action, None);
}

#[test]
fn zoom_chip_click_percent_readout_consumes_without_action() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);

    // The passive NN% readout sits between the ⊖ and ⊕ buttons: inside the
    // pill but off any button, so a click is consumed without an action (and
    // never falls through to the canvas).
    let (x, y) = {
        let layout = input.zoom_chip_layout().expect("layout");
        let out = layout
            .buttons
            .iter()
            .find(|b| b.kind == ZoomChipButtonKind::Out)
            .expect("out button");
        let in_button = layout
            .buttons
            .iter()
            .find(|b| b.kind == ZoomChipButtonKind::In)
            .expect("in button");
        let x = ((out.x + out.width + in_button.x) / 2.0).round() as i32;
        let y = (layout.pill_y + layout.pill_height / 2.0).round() as i32;
        (x, y)
    };
    assert!(input.zoom_chip_contains(x, y));
    assert_eq!(
        input
            .zoom_chip_layout()
            .unwrap()
            .button_at(x as f64, y as f64),
        None
    );

    // A press on a real button (⊖) that releases over the passive % is inside
    // the pill but off the pressed button, so it is consumed without an action
    // (and never falls through to the canvas). The passive % itself carries no
    // button kind, so it can never be the pressed target that fires.
    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::Out, x, y);
    assert!(hit);
    assert_eq!(action, None);
}

#[test]
fn zoom_chip_click_outside_is_ignored() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);

    let (hit, action) = input.check_zoom_chip_click(ZoomChipButtonKind::In, 5, 5);
    assert!(!hit);
    assert_eq!(action, None);
}

#[test]
fn zoom_chip_press_routing_consumes_left_press() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    // A left press over the chip must never start a stroke, and records the
    // pressed button for the same-button release contract.
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(matches!(input.state, DrawingState::Idle));
    assert_eq!(input.boards.active_frame().shapes.len(), 0);
    assert_eq!(
        input.zoom_chip_press_pending,
        ZoomChipPress::Button(ZoomChipButtonKind::In)
    );

    // With zoom actions disabled the press draws through as before.
    input.show_zoom_actions = false;
    update_chip_layout(&mut input, 1280, 720);
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert!(matches!(input.state, DrawingState::Drawing { .. }));
}

#[test]
fn tablet_path_press_release_dispatches_zoom_action() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    // Tablet input routes presses directly through the pointer chain: the
    // press consumes without dispatching, recording the pressed button...
    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert_eq!(
        input.zoom_chip_press_pending,
        ZoomChipPress::Button(ZoomChipButtonKind::In)
    );
    assert_eq!(input.take_pending_zoom_action(), None);

    // ...and the matching release inside the same button dispatches ZoomIn
    // through the shared action path, leaving a pending zoom action for the
    // event loop to drain.
    input.on_mouse_release_with_canvas(MouseButton::Left, x, y, x, y);
    assert_eq!(input.zoom_chip_press_pending, ZoomChipPress::None);
    assert_eq!(input.take_pending_zoom_action(), Some(ZoomAction::In));
}

#[test]
fn tablet_path_release_outside_does_not_dispatch() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);

    input.on_mouse_press_with_canvas(MouseButton::Left, x, y, x, y);
    assert_eq!(
        input.zoom_chip_press_pending,
        ZoomChipPress::Button(ZoomChipButtonKind::In)
    );

    // Releasing outside the pill consumes the pending press without
    // dispatching, and the flag does not leak into later releases.
    input.on_mouse_release_with_canvas(MouseButton::Left, 5, 5, 5, 5);
    assert_eq!(input.zoom_chip_press_pending, ZoomChipPress::None);
    assert_eq!(input.take_pending_zoom_action(), None);
}

#[test]
fn zoom_chip_ignored_while_eclipsing_overlay_open() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (x, y) = button_center(&input, ZoomChipButtonKind::In);
    assert!(input.zoom_chip_contains(x, y));

    // An overlay rendering above the chip suppresses its presses (also covers
    // an overlay opened between press and release: check_zoom_chip_click
    // shares the same guard).
    input.open_board_picker();
    assert!(!input.zoom_chip_contains(x, y));
    assert!(!input.check_zoom_chip_click(ZoomChipButtonKind::In, x, y).0);
    input.close_board_picker();
    assert!(input.zoom_chip_contains(x, y));
}

/// Same-button contract through the routing chain (the tablet/fallback path):
/// pressing ⊖, dragging to ⊕, and releasing there fires nothing — the pressed
/// and released buttons differ, so no zoom action leaks.
#[test]
fn press_out_drag_to_in_release_dispatches_nothing() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (ox, oy) = button_center(&input, ZoomChipButtonKind::Out);
    let (ix, iy) = button_center(&input, ZoomChipButtonKind::In);

    input.on_mouse_press_with_canvas(MouseButton::Left, ox, oy, ox, oy);
    assert_eq!(
        input.zoom_chip_press_pending,
        ZoomChipPress::Button(ZoomChipButtonKind::Out)
    );

    // Release over ⊕ (a different button): the pending press clears and no zoom
    // action is dispatched.
    input.on_mouse_release_with_canvas(MouseButton::Left, ix, iy, ix, iy);
    assert_eq!(input.zoom_chip_press_pending, ZoomChipPress::None);
    assert_eq!(input.take_pending_zoom_action(), None);
}

/// The passive `NN%` readout never fires: a press on it is swallowed (no
/// stroke) and records a `Passive` press (distinct from "no press"), so the
/// release stays consumed by the chip yet activates nothing — even over a real
/// button.
#[test]
fn passive_percent_press_never_fires_a_zoom_action() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);
    let (px, py) = passive_percent_center(&input);

    // Pressing the % swallows the press (no stroke under the chip) and records a
    // Passive press — the key distinction that keeps its release consumed.
    input.on_mouse_press_with_canvas(MouseButton::Left, px, py, px, py);
    assert!(matches!(input.state, DrawingState::Idle));
    assert_eq!(input.boards.active_frame().shapes.len(), 0);
    assert_eq!(input.zoom_chip_press_pending, ZoomChipPress::Passive);

    // Even releasing over a real button (⊕) fires nothing: a Passive press
    // carries no button to match. The release still consumes the pending flag.
    let (ix, iy) = button_center(&input, ZoomChipButtonKind::In);
    input.on_mouse_release_with_canvas(MouseButton::Left, ix, iy, ix, iy);
    assert_eq!(input.zoom_chip_press_pending, ZoomChipPress::None);
    assert_eq!(input.take_pending_zoom_action(), None);
}

/// A passive chip press captures its whole gesture: the release is consumed by
/// the chip and cannot fall through to finish an unrelated in-flight
/// interaction. Regression guard for the two-state `Option` flag, where a
/// passive press recorded `None` — indistinguishable from "no chip press" — so
/// its release leaked into normal routing and could complete another drag.
#[test]
fn passive_chip_release_does_not_finish_in_flight_interaction() {
    let mut input = create_test_input_state();
    update_chip_layout(&mut input, 1280, 720);

    // Simulate a freehand stroke left in flight (as if another modality owned
    // it): a leaked release would finish it and mutate the canvas.
    input.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 100,
        start_y: 100,
        points: vec![(100, 100), (150, 150)],
        point_thicknesses: vec![1.0, 1.0],
    };
    input.begin_pointer_drag(MouseButton::Left, None);
    assert_eq!(input.boards.active_frame().shapes.len(), 0);

    // Press the passive % area: swallowed, recorded as Passive (not None).
    let (px, py) = passive_percent_center(&input);
    input.on_mouse_press_with_canvas(MouseButton::Left, px, py, px, py);
    assert_eq!(input.zoom_chip_press_pending, ZoomChipPress::Passive);
    // The in-flight stroke is untouched by the swallowed press.
    assert!(matches!(input.state, DrawingState::Drawing { .. }));

    // Release anywhere: the chip consumes it, so the stroke is NOT finished —
    // the drawing state is left intact and nothing is committed. Under the old
    // two-state flag this release fell through and finished the stroke.
    input.on_mouse_release_with_canvas(MouseButton::Left, px, py, px, py);
    assert_eq!(input.zoom_chip_press_pending, ZoomChipPress::None);
    assert!(
        matches!(input.state, DrawingState::Drawing { .. }),
        "passive chip release must not finish the in-flight stroke"
    );
    assert_eq!(input.boards.active_frame().shapes.len(), 0);
    assert_eq!(input.take_pending_zoom_action(), None);
}
