use super::*;
use crate::config::PresenterToolBehavior;

#[test]
fn set_tool_override_clears_active_preset_and_resets_drawing_state() {
    let mut state = create_test_input_state();
    state.active_preset_slot = Some(2);
    state.needs_redraw = false;
    state.session_dirty = false;
    state.state = DrawingState::Drawing {
        tool: Tool::Pen,
        start_x: 10,
        start_y: 20,
        points: vec![(10, 20), (12, 24)],
        point_thicknesses: vec![3.0, 3.5],
    };

    assert!(state.set_tool_override(Some(Tool::Arrow)));
    assert_eq!(state.tool_override(), Some(Tool::Arrow));
    assert!(matches!(state.state, DrawingState::Idle));
    assert_eq!(state.active_preset_slot, None);
    assert!(state.needs_redraw);
    assert!(state.session_dirty);
}

#[test]
fn set_tool_override_preserves_text_input_state() {
    let mut state = create_test_input_state();
    state.state = DrawingState::TextInput {
        x: 4,
        y: 5,
        buffer: "hello".to_string(),
    };

    assert!(state.set_tool_override(Some(Tool::Rect)));
    assert_eq!(state.tool_override(), Some(Tool::Rect));
    assert!(matches!(
        &state.state,
        DrawingState::TextInput { x: 4, y: 5, buffer } if buffer == "hello"
    ));
}

#[test]
fn blur_tool_override_requests_frozen_capture_when_needed() {
    let mut state = create_test_input_state();

    assert!(state.set_tool_override(Some(Tool::Blur)));
    assert_eq!(state.tool_override(), Some(Tool::Blur));
    assert!(state.take_pending_frozen_toggle());
}

#[test]
fn presenter_locked_mode_rejects_non_highlight_tool_override() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Highlight)));
    state.presenter_mode = true;
    state.presenter_mode_config.tool_behavior = PresenterToolBehavior::ForceHighlightLocked;
    state.needs_redraw = false;
    state.session_dirty = false;

    assert!(!state.set_tool_override(Some(Tool::Pen)));
    assert_eq!(state.tool_override(), Some(Tool::Highlight));
    assert!(!state.needs_redraw);
    assert!(!state.session_dirty);
}

#[test]
fn set_thickness_for_active_tool_updates_eraser_size_when_eraser_is_active() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Eraser));

    assert!(state.set_thickness_for_active_tool(17.0));
    assert_eq!(state.eraser_size, 17.0);
    assert_eq!(state.current_thickness, 3.0);
}

#[test]
fn nudge_thickness_for_active_tool_clamps_pen_thickness() {
    let mut state = create_test_input_state();
    state.current_thickness = 49.0;

    assert!(state.nudge_thickness_for_active_tool(10.0));
    assert_eq!(state.current_thickness, 50.0);
}

#[test]
fn nudge_thickness_for_active_tool_clamps_eraser_size() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Eraser));
    state.eraser_size = 2.0;

    assert!(state.nudge_thickness_for_active_tool(-10.0));
    assert_eq!(state.eraser_size, 1.0);
}

#[test]
fn toggle_eraser_mode_round_trips_between_brush_and_stroke() {
    let mut state = create_test_input_state();
    assert_eq!(state.eraser_mode, EraserMode::Brush);

    assert!(state.toggle_eraser_mode());
    assert_eq!(state.eraser_mode, EraserMode::Stroke);

    assert!(state.toggle_eraser_mode());
    assert_eq!(state.eraser_mode, EraserMode::Brush);
}

#[test]
fn set_font_size_clamps_and_reports_noop_after_reaching_target() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.session_dirty = false;

    assert!(state.set_font_size(120.0));
    assert_eq!(state.current_font_size, 72.0);
    assert!(state.needs_redraw);
    assert!(state.session_dirty);

    state.needs_redraw = false;
    state.session_dirty = false;
    assert!(!state.set_font_size(72.0));
    assert!(!state.needs_redraw);
    assert!(!state.session_dirty);
}

#[test]
fn set_marker_opacity_clamps_and_reports_noop_after_reaching_target() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.session_dirty = false;

    assert!(state.set_marker_opacity(2.0));
    assert_eq!(state.marker_opacity, 0.9);
    assert!(state.needs_redraw);
    assert!(state.session_dirty);

    state.needs_redraw = false;
    state.session_dirty = false;
    assert!(!state.set_marker_opacity(0.9));
    assert!(!state.needs_redraw);
    assert!(!state.session_dirty);
}
