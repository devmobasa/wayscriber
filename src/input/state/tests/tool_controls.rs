use super::*;
use crate::config::{PresenterToolBehavior, PresetToolStatesConfig, ToolPresetConfig};
use crate::input::{DragBinding, DragToolBindings, PerToolDrawingSettings};
use crate::ui::toolbar::{ToolContext, ToolOptionsKind, ToolbarEvent, ToolbarSnapshot};

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
fn pick_screen_color_requests_backend_eyedropper_activation() {
    let mut state = create_test_input_state();

    state.handle_action(Action::PickScreenColor);

    assert!(state.take_pending_eyedropper_toggle());
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
fn toolbar_context_preserves_temporary_eraser_controls() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.left.shift_drag = DragBinding::from_tool(Tool::Eraser);
    assert!(state.set_drag_tool_bindings(bindings));
    assert!(state.set_tool_override(Some(Tool::Pen)));
    state.eraser_size = 17.0;

    state.on_key_press(Key::Shift);
    let snapshot = ToolbarSnapshot::from_input(&state);
    let context = ToolContext::from_snapshot(&snapshot);

    assert_eq!(snapshot.active_tool, Tool::Eraser);
    assert_eq!(snapshot.tool_override, Some(Tool::Pen));
    assert!(snapshot.thickness_targets_eraser);
    assert_eq!(snapshot.thickness, 17.0);
    assert_eq!(context.tool_options_kind, ToolOptionsKind::Eraser);
    assert_eq!(context.thickness_label, "Eraser size");
    assert!(context.show_eraser_mode);
}

#[test]
fn toolbar_context_matches_tool_profiles_for_each_tool() {
    let mut state = create_test_input_state();
    state.show_text_controls = false;
    state.show_marker_opacity_section = false;

    let cases = [
        (
            Tool::Select,
            false,
            false,
            ToolOptionsKind::None,
            "",
            false,
            false,
            false,
            false,
            false,
        ),
        (
            Tool::Pen,
            true,
            true,
            ToolOptionsKind::Stroke,
            "Thickness",
            false,
            false,
            false,
            false,
            false,
        ),
        (
            Tool::Line,
            true,
            true,
            ToolOptionsKind::Stroke,
            "Thickness",
            false,
            false,
            false,
            false,
            false,
        ),
        (
            Tool::Rect,
            true,
            true,
            ToolOptionsKind::Shape,
            "Thickness",
            true,
            false,
            false,
            false,
            false,
        ),
        (
            Tool::Ellipse,
            true,
            true,
            ToolOptionsKind::Shape,
            "Thickness",
            true,
            false,
            false,
            false,
            false,
        ),
        (
            Tool::Arrow,
            true,
            true,
            ToolOptionsKind::Arrow,
            "Thickness",
            false,
            true,
            false,
            false,
            false,
        ),
        (
            Tool::Blur,
            false,
            true,
            ToolOptionsKind::Stroke,
            "Blur",
            false,
            false,
            false,
            false,
            false,
        ),
        (
            Tool::Marker,
            true,
            true,
            ToolOptionsKind::Marker,
            "Thickness",
            false,
            false,
            false,
            false,
            true,
        ),
        (
            Tool::Highlight,
            false,
            false,
            ToolOptionsKind::None,
            "",
            false,
            false,
            false,
            false,
            false,
        ),
        (
            Tool::StepMarker,
            true,
            true,
            ToolOptionsKind::StepMarker,
            "Size",
            false,
            false,
            true,
            false,
            false,
        ),
        (
            Tool::Eraser,
            false,
            true,
            ToolOptionsKind::Eraser,
            "Eraser size",
            false,
            false,
            false,
            true,
            false,
        ),
    ];

    for (
        tool,
        needs_color,
        needs_thickness,
        tool_options_kind,
        thickness_label,
        show_fill_toggle,
        show_arrow_labels,
        show_step_counter,
        show_eraser_mode,
        show_marker_opacity,
    ) in cases
    {
        assert!(state.set_tool_override(Some(tool)));
        let snapshot = ToolbarSnapshot::from_input(&state);
        let context = ToolContext::from_snapshot(&snapshot);

        assert_eq!(context.needs_color, needs_color, "{tool:?} color");
        assert_eq!(
            context.needs_thickness, needs_thickness,
            "{tool:?} thickness"
        );
        assert_eq!(
            context.tool_options_kind, tool_options_kind,
            "{tool:?} options"
        );
        assert_eq!(context.thickness_label, thickness_label, "{tool:?} label");
        assert_eq!(context.show_fill_toggle, show_fill_toggle, "{tool:?} fill");
        assert_eq!(
            context.show_arrow_labels, show_arrow_labels,
            "{tool:?} arrow labels"
        );
        assert_eq!(
            context.show_step_counter, show_step_counter,
            "{tool:?} step counter"
        );
        assert_eq!(
            context.show_eraser_mode, show_eraser_mode,
            "{tool:?} eraser mode"
        );
        assert_eq!(
            context.show_marker_opacity, show_marker_opacity,
            "{tool:?} marker opacity"
        );
        assert!(!context.show_font_controls, "{tool:?} font controls");
    }
}

#[test]
fn toolbar_context_exposes_polygon_shape_controls() {
    let mut state = create_test_input_state();

    for tool in [
        Tool::Triangle,
        Tool::Parallelogram,
        Tool::Rhombus,
        Tool::RegularPolygon,
        Tool::FreeformPolygon,
    ] {
        assert!(state.set_tool_override(Some(tool)));
        let snapshot = ToolbarSnapshot::from_input(&state);
        let context = ToolContext::from_snapshot(&snapshot);

        assert_eq!(context.tool_options_kind, ToolOptionsKind::Shape);
        assert!(context.show_fill_toggle);
        assert_eq!(
            context.show_polygon_sides_control,
            tool == Tool::RegularPolygon,
            "{tool:?} sides control"
        );
    }
}

#[test]
fn polygon_side_controls_clamp_and_mark_session_dirty() {
    let mut state = create_test_input_state();
    state.clear_session_dirty();

    assert!(state.apply_toolbar_event(ToolbarEvent::SetPolygonSides(2)));
    assert_eq!(state.polygon_sides, 3);
    assert!(state.is_session_dirty());

    state.clear_session_dirty();
    assert!(state.apply_toolbar_event(ToolbarEvent::NudgePolygonSides(99)));
    assert_eq!(state.polygon_sides, 12);
    assert!(state.is_session_dirty());
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
    assert_eq!(
        state.color_for_tool(Tool::Pen),
        ColorSpec::from(pen_color).to_color()
    );
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
fn color_picker_copy_button_requests_copy_and_keeps_popup_open() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let x = (layout.copy_btn_x + layout.action_btn_size / 2.0) as i32;
    let y = (layout.copy_btn_y + layout.action_btn_size / 2.0) as i32;

    assert!(state.handle_color_picker_press(MouseButton::Left, x, y));
    assert!(state.handle_color_picker_popup_release_at(x, y));
    // The request is queued for the backend and the popup stays open.
    assert!(state.is_color_picker_popup_open());
    assert!(state.take_pending_copy_hex());
    assert!(!state.take_pending_paste_hex());
}

#[test]
fn color_picker_paste_button_requests_paste_and_keeps_popup_open() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let x = (layout.paste_btn_x + layout.action_btn_size / 2.0) as i32;
    let y = (layout.paste_btn_y + layout.action_btn_size / 2.0) as i32;

    assert!(state.handle_color_picker_press(MouseButton::Left, x, y));
    assert!(state.handle_color_picker_popup_release_at(x, y));
    assert!(state.is_color_picker_popup_open());
    assert!(state.take_pending_paste_hex());
    assert!(!state.take_pending_copy_hex());
}

#[test]
fn color_picker_action_release_requires_a_matching_press() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let copy_x = (layout.copy_btn_x + layout.action_btn_size / 2.0) as i32;
    let copy_y = (layout.copy_btn_y + layout.action_btn_size / 2.0) as i32;
    let paste_x = (layout.paste_btn_x + layout.action_btn_size / 2.0) as i32;
    let paste_y = (layout.paste_btn_y + layout.action_btn_size / 2.0) as i32;

    assert!(state.handle_color_picker_press(MouseButton::Left, copy_x, copy_y));
    assert!(state.handle_color_picker_popup_release_at(paste_x, paste_y));
    assert!(!state.take_pending_copy_hex());
    assert!(!state.take_pending_paste_hex());
}

#[test]
fn closing_color_picker_discards_queued_popup_paste() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.request_paste_hex();

    state.close_color_picker_popup(true);
    state.open_color_picker_popup();

    assert!(!state.take_pending_paste_hex());
}

#[test]
fn reopening_color_picker_invalidates_the_previous_paste_target() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.request_paste_hex();
    let target = state
        .take_pending_paste_hex_request()
        .expect("paste target");
    assert!(state.hex_paste_target_is_current(target));

    state.open_color_picker_popup();

    assert!(!state.hex_paste_target_is_current(target));
}

#[test]
fn color_picker_copy_request_keeps_the_pressed_color_after_close() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.color_picker_popup_set_from_gradient(0.3, 0.2);
    let copied = state.color_picker_popup_current_color().unwrap();

    state.request_copy_hex();
    state.close_color_picker_popup(true);

    assert_eq!(state.take_pending_copy_hex_request(), Some(copied));
}

#[test]
fn color_picker_action_buttons_occupy_distinct_regions() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");

    let copy = (
        layout.copy_btn_x + layout.action_btn_size / 2.0,
        layout.copy_btn_y + layout.action_btn_size / 2.0,
    );
    let paste = (
        layout.paste_btn_x + layout.action_btn_size / 2.0,
        layout.paste_btn_y + layout.action_btn_size / 2.0,
    );
    let eyedropper = (
        layout.eyedropper_btn_x + layout.action_btn_size / 2.0,
        layout.eyedropper_btn_y + layout.action_btn_size / 2.0,
    );

    // Each button's center hits only its own region, and none overlap the hex
    // input to its left.
    assert!(layout.point_in_copy_button(copy.0, copy.1));
    assert!(!layout.point_in_paste_button(copy.0, copy.1));
    assert!(!layout.point_in_hex_input(copy.0, copy.1));

    assert!(layout.point_in_paste_button(paste.0, paste.1));
    assert!(!layout.point_in_copy_button(paste.0, paste.1));
    assert!(!layout.point_in_eyedropper_button(paste.0, paste.1));

    assert!(layout.point_in_eyedropper_button(eyedropper.0, eyedropper.1));
    assert!(!layout.point_in_paste_button(eyedropper.0, eyedropper.1));

    // The cluster stays inside the panel's right edge.
    assert!(layout.eyedropper_btn_x + layout.action_btn_size <= layout.origin_x + layout.width);
}

#[test]
fn color_picker_set_color_updates_live_preview_and_buffer() {
    let mut state = create_test_input_state();
    let original = state.color_for_tool(Tool::Pen);
    state.open_color_picker_popup();

    let pasted = Color {
        r: 0.2,
        g: 0.4,
        b: 0.6,
        a: 1.0,
    };
    state.color_picker_popup_set_color(pasted);

    assert!(state.is_color_picker_popup_open());
    assert_eq!(state.color_picker_popup_current_color(), Some(pasted));
    assert_eq!(
        state.color_picker_popup_hex_buffer(),
        Some(crate::input::state::color_to_hex(pasted).as_str())
    );
    assert!(!state.color_picker_popup_is_hex_editing());
    // Previews live on the editing tool, like a gradient drag.
    assert_ne!(state.color_for_tool(Tool::Pen), original);
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
    assert_eq!(
        state.color_for_tool(Tool::Line),
        ColorSpec::from(line_color).to_color()
    );
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
fn save_preset_captures_all_tool_settings() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    let pen_color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };
    let line_color = Color {
        r: 0.4,
        g: 0.5,
        b: 0.6,
        a: 1.0,
    };
    let rect_color = Color {
        r: 0.9,
        g: 0.1,
        b: 0.2,
        a: 1.0,
    };
    let ellipse_color = Color {
        r: 0.2,
        g: 0.9,
        b: 0.4,
        a: 1.0,
    };
    let arrow_color = Color {
        r: 0.8,
        g: 0.3,
        b: 0.7,
        a: 1.0,
    };
    let blur_color = Color {
        r: 0.3,
        g: 0.3,
        b: 0.3,
        a: 1.0,
    };
    let marker_color = Color {
        r: 0.7,
        g: 0.8,
        b: 0.1,
        a: 1.0,
    };
    let step_color = Color {
        r: 0.2,
        g: 0.7,
        b: 0.9,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Pen)));
    assert!(state.set_color(pen_color));
    assert!(state.set_thickness(4.0));
    assert!(state.set_tool_override(Some(Tool::Line)));
    assert!(state.set_color(line_color));
    assert!(state.set_thickness(14.0));
    assert!(state.set_tool_override(Some(Tool::Rect)));
    assert!(state.set_color(rect_color));
    assert!(state.set_thickness(16.0));
    assert!(state.set_tool_override(Some(Tool::Ellipse)));
    assert!(state.set_color(ellipse_color));
    assert!(state.set_thickness(18.0));
    assert!(state.set_tool_override(Some(Tool::Arrow)));
    assert!(state.set_color(arrow_color));
    assert!(state.set_thickness(20.0));
    assert!(state.set_tool_override(Some(Tool::Blur)));
    assert!(state.set_color(blur_color));
    assert!(state.set_thickness(22.0));
    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert!(state.set_color(marker_color));
    assert!(state.set_thickness(24.0));
    assert!(state.set_tool_override(Some(Tool::StepMarker)));
    assert!(state.set_color(step_color));
    assert!(state.set_thickness(30.0));
    assert!(state.set_tool_override(Some(Tool::Eraser)));
    assert!(state.set_eraser_size(33.0));
    assert!(state.set_tool_override(Some(Tool::Line)));

    assert!(state.save_preset(1));
    let preset = state.presets[0].as_ref().expect("saved preset");
    let tool_settings = preset.tool_settings.as_ref().expect("tool settings");

    assert_eq!(preset.tool, Tool::Line);
    assert_eq!(preset.color, ColorSpec::from(line_color));
    assert_eq!(preset.size, 14.0);
    assert_eq!(tool_settings.pen.color, ColorSpec::from(pen_color));
    assert_eq!(tool_settings.pen.size, 4.0);
    assert_eq!(tool_settings.line.color, ColorSpec::from(line_color));
    assert_eq!(tool_settings.line.size, 14.0);
    assert_eq!(tool_settings.rect.color, ColorSpec::from(rect_color));
    assert_eq!(tool_settings.rect.size, 16.0);
    assert_eq!(tool_settings.ellipse.color, ColorSpec::from(ellipse_color));
    assert_eq!(tool_settings.ellipse.size, 18.0);
    assert_eq!(tool_settings.arrow.color, ColorSpec::from(arrow_color));
    assert_eq!(tool_settings.arrow.size, 20.0);
    assert_eq!(tool_settings.blur.color, ColorSpec::from(blur_color));
    assert_eq!(tool_settings.blur.size, 22.0);
    assert_eq!(tool_settings.marker.color, ColorSpec::from(marker_color));
    assert_eq!(tool_settings.marker.size, 24.0);
    assert_eq!(tool_settings.step_marker.color, ColorSpec::from(step_color));
    assert_eq!(tool_settings.step_marker.size, 30.0);
    assert_eq!(tool_settings.eraser_size, 33.0);
}

#[test]
fn save_preset_ignores_temporary_drag_modifier_tools() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 4;
    let pen_color = Color {
        r: 0.12,
        g: 0.34,
        b: 0.56,
        a: 1.0,
    };

    assert!(state.set_tool_override(Some(Tool::Pen)));
    assert!(state.set_color(pen_color));
    assert!(state.set_thickness(7.0));

    for (slot, modifiers, temporary_tool) in [
        (1, vec![Key::Shift], Tool::Line),
        (2, vec![Key::Ctrl], Tool::Rect),
        (3, vec![Key::Ctrl, Key::Shift], Tool::Arrow),
        (4, vec![Key::Tab], Tool::Ellipse),
    ] {
        for key in &modifiers {
            state.on_key_press(*key);
        }
        assert_eq!(state.active_tool(), temporary_tool);

        assert!(state.save_preset(slot));

        for key in modifiers.iter().rev() {
            state.on_key_release(*key);
        }
        let preset = state.presets[slot - 1].as_ref().expect("saved preset");
        assert_eq!(preset.tool, Tool::Pen);
        assert_eq!(preset.color, ColorSpec::from(pen_color));
        assert_eq!(preset.size, 7.0);
    }
}

#[test]
fn save_preset_without_override_uses_unmodified_drag_tool() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 1;
    let marker_color = Color {
        r: 0.72,
        g: 0.18,
        b: 0.42,
        a: 1.0,
    };

    state.drag_tool_bindings.left.drag = DragBinding::from_tool(Tool::Marker);
    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert!(state.set_color(marker_color));
    assert!(state.set_thickness(19.0));
    assert!(state.set_tool_override(None));

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Line);
    assert!(state.save_preset(1));
    state.on_key_release(Key::Shift);

    let preset = state.presets[0].as_ref().expect("saved preset");
    assert_eq!(preset.tool, Tool::Marker);
    assert_eq!(preset.color, ColorSpec::from(marker_color));
    assert_eq!(preset.size, 19.0);
}

#[test]
fn apply_full_preset_restores_all_tool_settings() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    let pen_color = Color {
        r: 0.1,
        g: 0.2,
        b: 0.3,
        a: 1.0,
    };
    let line_color = Color {
        r: 0.4,
        g: 0.5,
        b: 0.6,
        a: 1.0,
    };
    let rect_color = Color {
        r: 0.9,
        g: 0.1,
        b: 0.2,
        a: 1.0,
    };
    let ellipse_color = Color {
        r: 0.2,
        g: 0.9,
        b: 0.4,
        a: 1.0,
    };
    let arrow_color = Color {
        r: 0.8,
        g: 0.3,
        b: 0.7,
        a: 1.0,
    };
    let blur_color = Color {
        r: 0.3,
        g: 0.3,
        b: 0.3,
        a: 1.0,
    };
    let marker_color = Color {
        r: 0.7,
        g: 0.8,
        b: 0.1,
        a: 1.0,
    };
    let step_color = Color {
        r: 0.2,
        g: 0.7,
        b: 0.9,
        a: 1.0,
    };
    let mut settings = PerToolDrawingSettings::new(pen_color, 4.0);
    settings.line.color = line_color;
    settings.line.thickness = 14.0;
    settings.rect.color = rect_color;
    settings.rect.thickness = 16.0;
    settings.ellipse.color = ellipse_color;
    settings.ellipse.thickness = 18.0;
    settings.arrow.color = arrow_color;
    settings.arrow.thickness = 20.0;
    settings.blur.color = blur_color;
    settings.blur.thickness = 22.0;
    settings.marker.color = marker_color;
    settings.marker.thickness = 24.0;
    settings.step_marker.color = step_color;
    settings.step_marker.thickness = 30.0;
    let tool_settings = PresetToolStatesConfig::from_runtime(&settings, 33.0);

    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Marker,
        color: ColorSpec::from(marker_color),
        size: 24.0,
        tool_settings: Some(tool_settings),
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    });

    assert!(state.apply_preset(1));

    assert_eq!(state.active_tool(), Tool::Marker);
    assert_eq!(
        state.color_for_tool(Tool::Pen),
        ColorSpec::from(pen_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::Pen), 4.0);
    assert_eq!(
        state.color_for_tool(Tool::Line),
        ColorSpec::from(line_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::Line), 14.0);
    assert_eq!(
        state.color_for_tool(Tool::Rect),
        ColorSpec::from(rect_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::Rect), 16.0);
    assert_eq!(
        state.color_for_tool(Tool::Ellipse),
        ColorSpec::from(ellipse_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::Ellipse), 18.0);
    assert_eq!(
        state.color_for_tool(Tool::Arrow),
        ColorSpec::from(arrow_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::Arrow), 20.0);
    assert_eq!(
        state.color_for_tool(Tool::Blur),
        ColorSpec::from(blur_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::Blur), 22.0);
    assert_eq!(
        state.color_for_tool(Tool::Marker),
        ColorSpec::from(marker_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::Marker), 24.0);
    assert_eq!(
        state.color_for_tool(Tool::StepMarker),
        ColorSpec::from(step_color).to_color()
    );
    assert_eq!(state.thickness_for_tool(Tool::StepMarker), 30.0);
    assert_eq!(state.eraser_size, 33.0);
    assert_eq!(
        state.current_color,
        ColorSpec::from(marker_color).to_color()
    );
    assert_eq!(state.current_thickness, 24.0);
}

#[test]
fn toolbar_preset_preview_uses_nested_profile_for_active_preset_tool() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    let top_level_color = ColorSpec::Rgb([255, 0, 0]);
    let pen_color = ColorSpec::Rgb([10, 20, 30]);
    let marker_color = ColorSpec::Rgb([200, 180, 20]);
    let mut settings = PerToolDrawingSettings::new(pen_color.to_color(), 3.0);
    settings.marker.color = marker_color.to_color();
    settings.marker.thickness = 22.0;
    let tool_settings = PresetToolStatesConfig::from_runtime(&settings, 18.0);

    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Marker,
        color: top_level_color.clone(),
        size: 4.0,
        tool_settings: Some(tool_settings.clone()),
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    });
    state.presets[1] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Eraser,
        color: top_level_color,
        size: 4.0,
        tool_settings: Some(tool_settings),
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    });

    let snapshot = crate::ui::toolbar::ToolbarSnapshot::from_input(&state);
    let preset = snapshot.presets[0].as_ref().expect("preset preview");
    let eraser_preset = snapshot.presets[1].as_ref().expect("eraser preset preview");

    assert_eq!(preset.tool, Tool::Marker);
    assert_eq!(preset.color, marker_color.to_color());
    assert_eq!(preset.size, 22.0);
    assert_eq!(eraser_preset.tool, Tool::Eraser);
    assert_eq!(eraser_preset.color, pen_color.to_color());
    assert_eq!(eraser_preset.size, 18.0);
}

#[test]
fn legacy_preset_changes_only_selected_tool_settings() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    let pen_color = state.color_for_tool(Tool::Pen);
    let pen_thickness = state.thickness_for_tool(Tool::Pen);
    let line_color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    let marker_color = state.color_for_tool(Tool::Marker);
    let marker_thickness = state.thickness_for_tool(Tool::Marker);

    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Line,
        color: ColorSpec::from(line_color),
        size: 16.0,
        tool_settings: None,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    });

    assert!(state.apply_preset(1));

    assert_eq!(state.color_for_tool(Tool::Line), line_color);
    assert_eq!(state.thickness_for_tool(Tool::Line), 16.0);
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.thickness_for_tool(Tool::Pen), pen_thickness);
    assert_eq!(state.color_for_tool(Tool::Marker), marker_color);
    assert_eq!(state.thickness_for_tool(Tool::Marker), marker_thickness);
}

#[test]
fn legacy_step_marker_preset_uses_font_derived_size() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    state.set_tool_override(Some(Tool::StepMarker));
    assert!(state.set_thickness(30.0));
    state.set_tool_override(Some(Tool::Pen));

    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::StepMarker,
        color: ColorSpec::Name("blue".to_string()),
        size: 3.0,
        tool_settings: None,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: Some(48.0),
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    });

    assert!(state.apply_preset(1));

    assert_eq!(state.active_tool(), Tool::StepMarker);
    assert_eq!(state.current_font_size, 48.0);
    assert!((state.thickness_for_tool(Tool::StepMarker) - 28.8).abs() < 1e-9);
    assert!((state.next_step_marker_label().size - 28.8).abs() < 1e-9);
}

#[test]
fn full_step_marker_preset_uses_captured_profile_size() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    let color = ColorSpec::Rgb([20, 40, 60]);
    let mut settings = PerToolDrawingSettings::new(ColorSpec::Rgb([255, 0, 0]).to_color(), 4.0);
    settings.step_marker.color = color.to_color();
    settings.step_marker.thickness = 30.0;

    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::StepMarker,
        color: ColorSpec::Name("blue".to_string()),
        size: 3.0,
        tool_settings: Some(PresetToolStatesConfig::from_runtime(&settings, 18.0)),
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: Some(48.0),
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        polygon_sides: None,
        show_status_bar: None,
        drag_tools: None,
    });

    assert!(state.apply_preset(1));

    assert_eq!(state.active_tool(), Tool::StepMarker);
    assert_eq!(state.current_font_size, 48.0);
    assert_eq!(state.color_for_tool(Tool::StepMarker), color.to_color());
    assert_eq!(state.thickness_for_tool(Tool::StepMarker), 30.0);
    assert_eq!(state.next_step_marker_label().size, 30.0);
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
fn set_font_descriptor_marks_session_dirty_and_reports_noop_when_unchanged() {
    let mut state = create_test_input_state();
    let font = FontDescriptor::new(
        "Monospace".to_string(),
        "normal".to_string(),
        "italic".to_string(),
    );
    state.needs_redraw = false;
    state.session_dirty = false;

    assert!(state.set_font_descriptor(font.clone()));
    assert_eq!(state.font_descriptor, font);
    assert!(state.needs_redraw);
    assert!(state.session_dirty);

    state.needs_redraw = false;
    state.session_dirty = false;
    assert!(!state.set_font_descriptor(font));
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
