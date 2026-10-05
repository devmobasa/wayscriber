use super::*;

/// The modal term stops exactly where the tip and motion guards stop: once
/// the selector is on screen. While a capture is still pending the canvas
/// keeps the pen, so a stroke drawn then must keep its pressure — otherwise
/// a capture that fails would leave a silently flat stroke behind.
#[test]
fn stylus_pressure_stops_at_the_selector_not_at_the_request() {
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::state::{EyedropperCaptureSource, RegionPurposeTag, ScreenCaptureSource};

    let fresh_contact = |state: &_| drop_stylus_pressure(true, false, state);

    let mut state = make_test_input_state();
    assert!(!fresh_contact(&state));

    state.set_region_pending_capture(RegionPurposeTag::Ocr, 1, ScreenCaptureSource::Frozen);
    assert!(
        !fresh_contact(&state),
        "a stroke drawn while the capture is pending is still a real stroke"
    );
    state.activate_region_with(
        &crate::draw::TextMeasurer::default(),
        RegionPurposeTag::Ocr,
        1,
    );
    assert!(fresh_contact(&state));
    state.start_region_selection(RegionInputSource::Stylus, (10.0, 10.0));
    assert!(fresh_contact(&state));
    state.cancel_region_ui_only();
    assert!(!fresh_contact(&state));

    state.set_eyedropper_pending_capture(EyedropperCaptureSource::Frozen);
    assert!(!fresh_contact(&state));
    state.activate_eyedropper_with(&crate::draw::TextMeasurer::default(), Some(1));
    assert!(fresh_contact(&state));
    state.cancel_eyedropper();
    assert!(!fresh_contact(&state));
}

/// A contact disowned by a modal keeps arriving until the pen lifts, and the
/// pending-capture allowance above must not readmit it: the canvas holds the
/// pen again during the wait, but this contact is not the user drawing.
#[test]
fn a_retired_contact_never_reaches_the_tool_whatever_the_modal_is_doing() {
    use crate::input::state::test_support::make_test_input_state;
    use crate::input::state::{RegionPurposeTag, ScreenCaptureSource};

    let mut state = make_test_input_state();

    // The window the previous test allows, and the one that matters here.
    state.set_region_pending_capture(RegionPurposeTag::Ocr, 1, ScreenCaptureSource::Frozen);
    assert!(!drop_stylus_pressure(true, false, &state));
    assert!(drop_stylus_pressure(true, true, &state));

    // And it outlives the modal: cancelling before activation leaves the pen
    // physically down with no modal to blame.
    state.cancel_region_ui_only();
    assert!(!drop_stylus_pressure(true, false, &state));
    assert!(drop_stylus_pressure(true, true, &state));

    // Off the overlay nothing is ours either way.
    assert!(drop_stylus_pressure(false, false, &state));
}

#[test]
fn stylus_cursor_damage_rect_covers_cursor_area() {
    let rect = stylus_cursor_damage_rect((100.2, 80.7), 400, 300).expect("rect");

    assert_eq!(
        rect,
        Rect::new(
            100 - STYLUS_CURSOR_DAMAGE_RADIUS,
            81 - STYLUS_CURSOR_DAMAGE_RADIUS,
            STYLUS_CURSOR_DAMAGE_RADIUS * 2,
            STYLUS_CURSOR_DAMAGE_RADIUS * 2,
        )
        .unwrap()
    );
}

#[test]
fn stylus_cursor_damage_rect_clamps_to_surface() {
    let rect = stylus_cursor_damage_rect((4.0, 3.0), 400, 300).expect("rect");

    assert_eq!(rect.x, 0);
    assert_eq!(rect.y, 0);
    assert_eq!(rect.width, 68);
    assert_eq!(rect.height, 67);
}

#[test]
fn stylus_cursor_damage_rect_ignores_empty_surface() {
    assert_eq!(stylus_cursor_damage_rect((10.0, 10.0), 0, 300), None);
    assert_eq!(stylus_cursor_damage_rect((10.0, 10.0), 400, 0), None);
}
