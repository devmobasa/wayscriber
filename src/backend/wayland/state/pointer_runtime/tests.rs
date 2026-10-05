use super::{PendingChromePress, PointerRuntime, TouchState, TouchTarget};
use crate::{
    input::state::{RegionInputSource, ToastPress},
    ui::{OnboardingCardPress, ZoomChipPress},
};
use smithay_client_toolkit::seat::pointer::CursorIcon;

#[test]
fn pointer_attachment_reset_clears_shape_and_hidden_state() {
    let mut runtime = PointerRuntime::new();
    runtime.current_pointer_shape = Some(CursorIcon::Crosshair);
    runtime.cursor_hidden = true;

    runtime.reset_cursor_cache();

    assert_eq!(runtime.current_pointer_shape, None);
    assert!(!runtime.cursor_hidden);
}

#[test]
fn hide_and_show_transitions_are_idempotent() {
    let mut runtime = PointerRuntime::new();

    assert!(runtime.mark_cursor_hidden());
    assert!(!runtime.mark_cursor_hidden());
    assert!(runtime.show_cursor());
    assert!(!runtime.show_cursor());
}

#[test]
fn active_touch_rejects_a_second_contact_and_foreign_end() {
    let mut touch = TouchState::default();
    assert!(touch.begin(7, (10.0, 20.0), TouchTarget::Canvas));
    assert!(!touch.begin(8, (30.0, 40.0), TouchTarget::Toolbar));
    assert_eq!(touch.end(8), None);
    assert_eq!(touch.end(7), Some(((10.0, 20.0), TouchTarget::Canvas)));
    assert_eq!(touch.end(7), None);
}

#[test]
fn active_touch_updates_only_the_owned_contact() {
    let mut touch = TouchState::default();
    assert!(touch.begin(7, (10.0, 20.0), TouchTarget::Canvas));

    assert!(!touch.update_position(8, (30.0, 40.0)));
    assert!(touch.update_position(7, (50.0, 60.0)));
    assert_eq!(touch.end(7), Some(((50.0, 60.0), TouchTarget::Canvas)));
}

#[test]
fn chrome_press_priority_keeps_the_first_target() {
    let mut press = PendingChromePress::default();
    let toast = ToastPress::body(7);

    assert!(press.arm_toast(toast));
    assert!(!press.arm_status_hud());
    assert!(!press.arm_zoom_chip(ZoomChipPress::Passive));
    assert_eq!(press.take_toast(), Some(toast));
    assert_eq!(press.take_toast(), None);
}

#[test]
fn an_onboarding_card_press_is_owned_until_its_release_takes_it() {
    let mut runtime = PointerRuntime::new();

    assert!(runtime.arm_onboarding_card_press(OnboardingCardPress::Body));
    assert!(!runtime.arm_toast_press(ToastPress::body(7)));
    assert_eq!(
        runtime.take_onboarding_card_press(),
        Some(OnboardingCardPress::Body)
    );
    assert_eq!(runtime.take_onboarding_card_press(), None);

    assert!(runtime.arm_onboarding_card_press(OnboardingCardPress::Body));
    runtime.clear_chrome_press();
    assert_eq!(runtime.take_onboarding_card_press(), None);
}

#[test]
fn clearing_chrome_press_preserves_both_release_latches() {
    let mut runtime = PointerRuntime::new();
    assert!(runtime.arm_status_hud_press());
    runtime.suppress_release(RegionInputSource::Pointer);
    runtime.suppress_release(RegionInputSource::Touch);

    runtime.clear_chrome_press();
    assert!(!runtime.take_status_hud_press());

    assert!(runtime.arm_toast_press(ToastPress::body(7)));
    runtime.clear_chrome_press();
    assert_eq!(runtime.take_toast_press(), None);

    assert!(runtime.arm_zoom_chip_press(ZoomChipPress::Passive));
    runtime.clear_chrome_press();
    assert_eq!(runtime.take_zoom_chip_press(), ZoomChipPress::None);
    assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
    assert!(runtime.take_suppressed_release(RegionInputSource::Touch));
}

#[test]
fn release_suppression_is_owned_by_its_source() {
    let mut runtime = PointerRuntime::new();
    runtime.suppress_release(RegionInputSource::Pointer);
    runtime.suppress_release(RegionInputSource::Touch);
    runtime.clear_suppressed_release(RegionInputSource::Touch);

    assert!(!runtime.take_suppressed_release(RegionInputSource::Touch));
    assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
    assert!(!runtime.take_suppressed_release(RegionInputSource::Pointer));
    assert!(!runtime.take_suppressed_release(RegionInputSource::Stylus));
}

#[test]
fn pointer_cleanup_preserves_a_pending_touch_release() {
    let mut runtime = PointerRuntime::new();
    runtime.suppress_release(RegionInputSource::Pointer);
    runtime.suppress_release(RegionInputSource::Touch);

    assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
    runtime.clear_chrome_press();

    assert!(!runtime.take_suppressed_release(RegionInputSource::Pointer));
    assert!(runtime.take_suppressed_release(RegionInputSource::Touch));
    assert!(!runtime.take_suppressed_release(RegionInputSource::Touch));
}

#[test]
fn touch_cancellation_cleanup_preserves_a_pending_pointer_release() {
    let mut runtime = PointerRuntime::new();
    runtime.suppress_release(RegionInputSource::Pointer);
    runtime.suppress_release(RegionInputSource::Touch);

    runtime.clear_chrome_press();
    runtime.clear_suppressed_release(RegionInputSource::Touch);

    assert!(!runtime.take_suppressed_release(RegionInputSource::Touch));
    assert!(runtime.take_suppressed_release(RegionInputSource::Pointer));
}

#[test]
fn held_eyedropper_pointer_release_does_not_finish_a_new_touch_stroke() {
    assert_interleaved_release_keeps_stroke(RegionInputSource::Pointer, RegionInputSource::Touch);
}

#[test]
fn held_region_touch_release_does_not_finish_a_new_pointer_stroke() {
    assert_interleaved_release_keeps_stroke(RegionInputSource::Touch, RegionInputSource::Pointer);
}

fn assert_interleaved_release_keeps_stroke(
    consumed_source: RegionInputSource,
    drawing_source: RegionInputSource,
) {
    use crate::input::{MouseButton, state::DrawingState};

    let measurer = crate::draw::TextMeasurer::default();
    let ui_engine = crate::ui_text::UiTextEngine::default();
    let resources = crate::input::state::InputTextResources {
        measurer: &measurer,
        ui_engine: &ui_engine,
    };
    let mut runtime = PointerRuntime::new();
    let mut input = crate::input::state::test_support::make_test_input_state();

    // The modal press was consumed, but that device remains held while the
    // other device starts drawing. Both canvas press handlers reset chrome.
    runtime.suppress_release(consumed_source);
    runtime.clear_chrome_press();
    input.on_mouse_press_with_canvas_and_resources(resources, MouseButton::Left, 10, 20, 10, 20);
    input.on_mouse_motion_with_canvas_and_resources(resources, 30, 40, 30, 40);
    assert!(matches!(input.state, DrawingState::Drawing { .. }));

    // Exercise the release gate shared by the pointer and touch handlers.
    // Falling through here would commit the other device's unfinished stroke.
    if runtime.take_suppressed_release(consumed_source) {
        runtime.clear_chrome_press();
    } else {
        input.on_mouse_release_with_canvas_and_resources(
            resources,
            MouseButton::Left,
            30,
            40,
            30,
            40,
        );
    }
    assert!(matches!(input.state, DrawingState::Drawing { .. }));
    assert!(input.boards.active_frame().shapes.is_empty());

    assert!(!runtime.take_suppressed_release(drawing_source));
    input.on_mouse_release_with_canvas_and_resources(resources, MouseButton::Left, 50, 60, 50, 60);
    assert!(matches!(input.state, DrawingState::Idle));
    assert_eq!(input.boards.active_frame().shapes.len(), 1);
}

#[test]
fn chrome_press_targets_are_taken_once() {
    let mut runtime = PointerRuntime::new();
    assert!(runtime.arm_zoom_chip_press(ZoomChipPress::Passive));

    assert_eq!(runtime.take_zoom_chip_press(), ZoomChipPress::Passive);
    assert_eq!(runtime.take_zoom_chip_press(), ZoomChipPress::None);
}

#[test]
fn board_pan_advance_uses_and_updates_the_previous_sample() {
    let mut runtime = PointerRuntime::new();
    runtime.start_board_pan((10.0, 20.0));

    assert_eq!(runtime.advance_board_pan((13.5, 18.0)), (3.5, -2.0));
    assert_eq!(runtime.advance_board_pan((15.0, 22.0)), (1.5, 4.0));
}

#[test]
fn pointer_position_round_trips() {
    let mut runtime = PointerRuntime::new();
    runtime.set_position((17, 23));

    assert_eq!(runtime.position(), (17, 23));
}

#[test]
fn unlocking_an_empty_runtime_reports_no_transition() {
    let mut runtime = PointerRuntime::new();

    assert!(!runtime.unlock());
}
