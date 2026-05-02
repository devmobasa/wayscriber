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
    assert!(state.set_thickness(49.0));

    assert!(state.nudge_thickness_for_active_tool(10.0));
    assert_eq!(state.current_thickness, 50.0);
}

#[test]
fn tool_color_and_thickness_are_independent_between_pen_and_marker() {
    let mut state = create_test_input_state();
    let pen_color = state.color_for_tool(Tool::Pen);
    let pen_thickness = state.thickness_for_tool(Tool::Pen);
    let marker_color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert!(state.set_color(marker_color));
    assert!(state.set_thickness(24.0));

    assert_eq!(state.color_for_tool(Tool::Marker), marker_color);
    assert_eq!(state.thickness_for_tool(Tool::Marker), 24.0);
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.thickness_for_tool(Tool::Pen), pen_thickness);

    assert!(state.set_tool_override(Some(Tool::Pen)));
    assert_eq!(state.current_color, pen_color);
    assert_eq!(state.current_thickness, pen_thickness);

    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert_eq!(state.current_color, marker_color);
    assert_eq!(state.current_thickness, 24.0);
}

#[test]
fn increase_thickness_action_changes_marker_width_not_marker_opacity() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert!(state.set_thickness(24.0));
    let original_opacity = state.marker_opacity;
    let pen_thickness = state.thickness_for_tool(Tool::Pen);

    state.handle_action(crate::config::Action::IncreaseThickness);

    assert_eq!(state.thickness_for_tool(Tool::Marker), 25.0);
    assert_eq!(state.thickness_for_tool(Tool::Pen), pen_thickness);
    assert_eq!(state.marker_opacity, original_opacity);
}

#[test]
fn modifier_release_resyncs_current_settings_to_base_tool() {
    let mut state = create_test_input_state();
    let pen_color = state.color_for_tool(Tool::Pen);
    let pen_thickness = state.thickness_for_tool(Tool::Pen);
    let line_color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Line)));
    assert!(state.set_color(line_color));
    assert!(state.set_thickness(14.0));
    assert!(state.set_tool_override(Some(Tool::Pen)));

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Line);
    assert_eq!(state.current_color, line_color);
    assert_eq!(state.current_thickness, 14.0);

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 10, 10);
    assert_eq!(state.current_color, line_color);
    assert_eq!(state.current_thickness, 14.0);

    state.on_key_release(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Pen);
    assert_eq!(state.current_color, pen_color);
    assert_eq!(state.current_thickness, pen_thickness);
}

#[test]
fn reset_modifiers_resyncs_current_settings_to_base_tool() {
    let mut state = create_test_input_state();
    let pen_color = state.color_for_tool(Tool::Pen);
    let pen_thickness = state.thickness_for_tool(Tool::Pen);
    let line_color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Line)));
    assert!(state.set_color(line_color));
    assert!(state.set_thickness(14.0));
    assert!(state.set_tool_override(Some(Tool::Pen)));

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Line);
    assert_eq!(state.current_color, line_color);
    assert_eq!(state.current_thickness, 14.0);

    state.reset_modifiers();
    assert_eq!(state.active_tool(), Tool::Pen);
    assert_eq!(state.current_color, pen_color);
    assert_eq!(state.current_thickness, pen_thickness);
}

#[test]
fn sync_modifiers_resyncs_current_settings_to_compositor_tool() {
    let mut state = create_test_input_state();
    let pen_color = state.color_for_tool(Tool::Pen);
    let pen_thickness = state.thickness_for_tool(Tool::Pen);
    let line_color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Line)));
    assert!(state.set_color(line_color));
    assert!(state.set_thickness(14.0));
    assert!(state.set_tool_override(Some(Tool::Pen)));

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Line);
    assert_eq!(state.current_color, line_color);
    assert_eq!(state.current_thickness, 14.0);

    state.sync_modifiers(false, false, false);
    assert_eq!(state.active_tool(), Tool::Pen);
    assert_eq!(state.current_color, pen_color);
    assert_eq!(state.current_thickness, pen_thickness);
}

#[test]
fn canceling_color_picker_restores_color_without_dirtying_session_or_preset() {
    let mut state = create_test_input_state();
    let original = state.color_for_tool(Tool::Pen);
    state.active_preset_slot = Some(1);
    state.session_dirty = false;

    state.open_color_picker_popup();
    state.color_picker_popup_set_from_gradient(0.6, 0.1);
    assert_ne!(state.color_for_tool(Tool::Pen), original);
    state.close_color_picker_popup(true);

    assert_eq!(state.color_for_tool(Tool::Pen), original);
    assert_eq!(state.current_color, original);
    assert_eq!(state.active_preset_slot, Some(1));
    assert!(!state.session_dirty);
}

#[test]
fn color_picker_cancel_restores_opening_modifier_tool_after_modifier_release() {
    let mut state = create_test_input_state();
    let pen_color = state.color_for_tool(Tool::Pen);
    let line_color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Line)));
    assert!(state.set_color(line_color));
    assert!(state.set_tool_override(Some(Tool::Pen)));

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Line);
    state.open_color_picker_popup();
    state.on_key_release(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Pen);
    assert_eq!(state.current_color, pen_color);

    state.color_picker_popup_set_from_gradient(0.3, 0.2);
    assert_ne!(state.color_for_tool(Tool::Line), line_color);
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.current_color, pen_color);

    state.close_color_picker_popup(true);
    assert_eq!(state.color_for_tool(Tool::Line), line_color);
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.current_color, pen_color);
}

#[test]
fn color_picker_apply_updates_opening_modifier_tool_after_modifier_release() {
    let mut state = create_test_input_state();
    let pen_color = state.color_for_tool(Tool::Pen);
    let line_color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Line)));
    assert!(state.set_color(line_color));
    assert!(state.set_tool_override(Some(Tool::Pen)));
    state.session_dirty = false;

    state.on_key_press(Key::Shift);
    state.open_color_picker_popup();
    state.on_key_release(Key::Shift);

    state.color_picker_popup_set_from_gradient(0.3, 0.2);
    let applied = state.color_picker_popup_current_color().unwrap();
    state.apply_color_picker_popup();

    assert_eq!(state.color_for_tool(Tool::Line), applied);
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.current_color, pen_color);
    assert!(state.session_dirty);
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
