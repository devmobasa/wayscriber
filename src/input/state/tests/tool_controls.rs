use super::*;
use crate::config::{PresenterToolBehavior, PresetToolStatesConfig, ToolPresetConfig};
use crate::draw::{ArrowStyle, BlurStyle};
use crate::input::{DragBinding, DragToolBindings, PerToolDrawingSettings};
use crate::ui::toolbar::model::{StylePillControl, StylePillSpec, TopStripPlan};
use crate::ui::toolbar::{ToolContext, ToolOptionsKind, ToolbarEvent, ToolbarSnapshot};

#[test]
fn set_tool_override_clears_active_preset_and_resets_drawing_state() {
    let mut state = create_test_input_state();
    state.preset_slots.restore_active(Some(2));
    state.needs_redraw = false;
    state.clear_session_dirty();
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
    assert_eq!(state.preset_slots.active(), None);
    assert!(state.needs_redraw);
    assert!(state.is_session_dirty());
}

#[test]
fn set_tool_override_preserves_text_input_state() {
    let mut state = create_test_input_state();
    state.state = DrawingState::text_input(4, 5, "hello".to_string());

    assert!(state.set_tool_override(Some(Tool::Rect)));
    assert_eq!(state.tool_override(), Some(Tool::Rect));
    assert!(matches!(
        &state.state,
        DrawingState::TextInput { x: 4, y: 5, buffer, .. } if buffer == "hello"
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
fn black_out_tool_override_does_not_request_frozen_capture() {
    let mut state = create_test_input_state();
    assert!(state.set_blur_style(BlurStyle::BlackOut));

    assert!(state.set_tool_override(Some(Tool::Blur)));

    assert_eq!(state.tool_override(), Some(Tool::Blur));
    assert!(!state.take_pending_frozen_toggle());
}

#[test]
fn cycling_from_black_out_to_sampling_blur_requests_frozen_capture() {
    let mut state = create_test_input_state();
    assert!(state.set_blur_style(BlurStyle::BlackOut));
    assert!(state.set_tool_override(Some(Tool::Blur)));
    assert!(!state.take_pending_frozen_toggle());

    assert!(state.cycle_blur_style());

    assert_eq!(state.style.blur_style, BlurStyle::Gaussian);
    assert!(state.take_pending_frozen_toggle());
}

#[test]
fn pick_screen_color_requests_backend_eyedropper_activation() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_test_input_state();

    state.handle_action_with_resources(test_text_resources, Action::PickScreenColor);

    assert!(state.take_pending_eyedropper_toggle());
}

#[test]
fn presenter_locked_mode_rejects_non_highlight_tool_override() {
    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Highlight)));
    state.override_presenter_mode_for_test(true);
    state.presenter_mode_config_mut_for_test().tool_behavior =
        PresenterToolBehavior::ForceHighlightLocked;
    state.needs_redraw = false;
    state.clear_session_dirty();

    assert!(!state.set_tool_override(Some(Tool::Pen)));
    assert_eq!(state.tool_override(), Some(Tool::Highlight));
    assert!(!state.needs_redraw);
    assert!(!state.is_session_dirty());
}

#[test]
fn set_thickness_for_active_tool_updates_eraser_size_when_eraser_is_active() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Eraser));

    assert!(state.set_thickness_for_active_tool(17.0));
    assert_eq!(state.style.eraser_size, 17.0);
    assert_eq!(state.style.current_thickness, 3.0);
}

#[test]
fn toolbar_context_preserves_temporary_eraser_controls() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.left.shift_drag = DragBinding::from_tool(Tool::Eraser);
    assert!(state.set_drag_tool_bindings(bindings));
    assert!(state.set_tool_override(Some(Tool::Pen)));
    state.style.eraser_size = 17.0;

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
    state.ui_visibility.show_text_controls = false;
    state.ui_visibility.show_marker_opacity_section = false;

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
        (
            Tool::Spotlight,
            false,
            false,
            ToolOptionsKind::Spotlight,
            "Spotlight",
            false,
            false,
            false,
            false,
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
fn toolbar_spotlight_magnification_clamps_and_updates_the_snapshot() {
    let mut state = create_test_input_state();
    state.clear_session_dirty();

    assert!(state.apply_toolbar_event(ToolbarEvent::SetSpotlightMagnification(2.13)));
    assert_eq!(state.style.spotlight_magnification, 2.25);
    assert!(state.apply_toolbar_event(ToolbarEvent::SetSpotlightMagnification(9.0)));
    assert_eq!(state.style.spotlight_magnification, 4.0);
    assert_eq!(
        ToolbarSnapshot::from_input(&state).spotlight_magnification,
        4.0
    );
    assert!(state.is_session_dirty());
    assert!(state.apply_toolbar_event(ToolbarEvent::SetSpotlightMagnification(f64::NAN)));
    assert_eq!(state.style.spotlight_magnification, 1.0);
}

#[test]
fn changing_the_next_shape_default_does_not_warn_about_sources() {
    let mut state = create_test_input_state();
    state.style.spotlight_magnification = 1.0;

    // The slider changes what the *next* Spotlight will use. No Spotlight has
    // been created or edited, so there is no action to warn about; the style
    // control's inline unavailable state carries this case, and a toast here
    // would fire on every step of a slider drag.
    assert!(state.set_spotlight_magnification(2.0));
    assert!(!state.take_pending_spotlight_magnifier_feedback());

    assert!(state.set_spotlight_magnification(1.0));
    assert!(!state.take_pending_spotlight_magnifier_feedback());
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
    assert_eq!(state.style.polygon_sides, 3);
    assert!(state.is_session_dirty());

    state.clear_session_dirty();
    assert!(state.apply_toolbar_event(ToolbarEvent::NudgePolygonSides(99)));
    assert_eq!(state.style.polygon_sides, 12);
    assert!(state.is_session_dirty());
}

#[test]
fn nudge_thickness_for_active_tool_clamps_pen_thickness() {
    let mut state = create_test_input_state();
    assert!(state.set_thickness(49.0));

    assert!(state.nudge_thickness_for_active_tool(10.0));
    assert_eq!(state.style.current_thickness, 50.0);
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
    assert_eq!(state.style.current_color, pen_color);
    assert_eq!(state.style.current_thickness, pen_thickness);

    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert_eq!(state.style.current_color, marker_color);
    assert_eq!(state.style.current_thickness, 24.0);
}

#[test]
fn increase_thickness_action_changes_marker_width_not_marker_opacity() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_test_input_state();
    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert!(state.set_thickness(24.0));
    let original_opacity = state.style.marker_opacity;
    let pen_thickness = state.thickness_for_tool(Tool::Pen);

    state.handle_action_with_resources(
        test_text_resources,
        crate::config::Action::IncreaseThickness,
    );

    assert_eq!(state.thickness_for_tool(Tool::Marker), 25.0);
    assert_eq!(state.thickness_for_tool(Tool::Pen), pen_thickness);
    assert_eq!(state.style.marker_opacity, original_opacity);
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
    assert_eq!(state.style.current_color, line_color);
    assert_eq!(state.style.current_thickness, 14.0);

    state.on_mouse_press(MouseButton::Left, 0, 0);
    state.on_mouse_release(MouseButton::Left, 10, 10);
    assert_eq!(state.style.current_color, line_color);
    assert_eq!(state.style.current_thickness, 14.0);

    state.on_key_release(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Pen);
    assert_eq!(state.style.current_color, pen_color);
    assert_eq!(state.style.current_thickness, pen_thickness);
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
    assert_eq!(state.style.current_color, line_color);
    assert_eq!(state.style.current_thickness, 14.0);

    state.reset_modifiers();
    assert_eq!(state.active_tool(), Tool::Pen);
    assert_eq!(state.style.current_color, pen_color);
    assert_eq!(state.style.current_thickness, pen_thickness);
}

#[test]
fn super_does_not_change_drag_tool_priority() {
    let mut state = create_test_input_state();
    state.on_key_press(Key::Super);
    assert!(state.modifiers.logo);
    assert_eq!(state.active_tool(), Tool::Pen);

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Line);

    state.on_key_press(Key::Ctrl);
    assert_eq!(state.active_tool(), Tool::Arrow);

    state.on_key_release(Key::Super);
    assert!(!state.modifiers.logo);
    assert_eq!(state.active_tool(), Tool::Arrow);
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
    assert_eq!(state.style.current_color, line_color);
    assert_eq!(state.style.current_thickness, 14.0);

    state.sync_modifiers(false, false, false, false);
    assert_eq!(state.active_tool(), Tool::Pen);
    assert_eq!(state.style.current_color, pen_color);
    assert_eq!(state.style.current_thickness, pen_thickness);
}

#[test]
fn canceling_color_picker_restores_color_without_dirtying_session_or_preset() {
    let mut state = create_test_input_state();
    let original = state.color_for_tool(Tool::Pen);
    state.preset_slots.restore_active(Some(1));
    state.clear_session_dirty();

    state.open_color_picker_popup();
    state.color_picker_popup_set_from_gradient(0.6, 0.1);
    assert_ne!(state.color_for_tool(Tool::Pen), original);
    state.close_color_picker_popup(true);

    assert_eq!(state.color_for_tool(Tool::Pen), original);
    assert_eq!(state.style.current_color, original);
    assert_eq!(state.preset_slots.active(), Some(1));
    assert!(!state.is_session_dirty());
}

#[test]
fn recoloring_a_swatch_previews_on_the_palette_and_leaves_the_tool_alone() {
    let mut state = create_test_input_state();
    let tool_color = state.color_for_tool(Tool::Pen);
    let slot_color = state.style.quick_colors.color_for_index(1).expect("slot 1");
    assert_ne!(tool_color, slot_color, "fixture needs distinct colors");
    state.clear_session_dirty();

    assert!(state.open_color_picker_popup_for_quick_color(1));
    assert_eq!(state.color_picker_popup_slot(), Some(1));
    // The popup starts on the slot's own color, not the tool's.
    assert_eq!(state.color_picker_popup_current_color(), Some(slot_color));

    state.color_picker_popup_set_from_gradient(0.6, 0.1);
    let picked = state
        .color_picker_popup_current_color()
        .expect("picked color");
    // The swatch tracks the drag so the toolbar shows the candidate...
    assert_eq!(state.style.quick_colors.color_for_index(1), Some(picked));
    // ...while the color being painted with is untouched.
    assert_eq!(state.color_for_tool(Tool::Pen), tool_color);

    state.close_color_picker_popup(true);
    assert_eq!(
        state.style.quick_colors.color_for_index(1),
        Some(slot_color)
    );
    assert_eq!(state.color_for_tool(Tool::Pen), tool_color);
    assert!(!state.is_session_dirty());
    assert!(state.active_toast().is_none());
}

/// Accepting keeps the swatch and hands the slot to the backend to write.
///
/// `InputState` owns neither the configuration nor the filesystem, so the
/// accept records what it decided; the toast belongs to the backend, which
/// knows whether the write landed.
#[test]
fn accepting_a_recolor_keeps_the_swatch_and_queues_the_durable_write() {
    let mut state = create_test_input_state();
    let tool_color = state.color_for_tool(Tool::Pen);
    state.preset_slots.restore_active(Some(1));
    state.clear_session_dirty();

    assert!(state.open_color_picker_popup_for_quick_color(2));
    state.color_picker_popup_set_from_gradient(0.35, 0.25);
    let picked = state
        .color_picker_popup_current_color()
        .expect("picked color");
    state.apply_color_picker_popup();

    assert!(!state.is_color_picker_popup_open());
    assert_eq!(state.style.quick_colors.color_for_index(2), Some(picked));
    assert_eq!(
        state.take_pending_quick_color_edit(),
        Some(crate::input::state::QuickColorEdit {
            index: 2,
            color: picked
        })
    );
    assert!(
        state.active_toast().is_none(),
        "the backend raises the toast once it knows whether the write landed"
    );
    // An unselected swatch's recolor is not a drawing change.
    assert_eq!(state.color_for_tool(Tool::Pen), tool_color);
    assert_eq!(state.preset_slots.active(), Some(1));
    assert!(!state.is_session_dirty());
}

#[test]
fn recoloring_the_swatch_in_use_moves_the_tool_color_with_it() {
    let mut state = create_test_input_state();
    let slot_color = state.style.quick_colors.color_for_index(3).expect("slot 3");
    assert!(
        state.apply_color_from_ui_with_measurer(&crate::draw::TextMeasurer::default(), slot_color)
    );
    state.preset_slots.restore_active(Some(2));
    state.clear_session_dirty();

    assert!(state.open_color_picker_popup_for_quick_color(3));
    state.color_picker_popup_set_from_gradient(0.8, 0.4);
    let picked = state
        .color_picker_popup_current_color()
        .expect("picked color");
    state.apply_color_picker_popup();

    // The selection ring cannot disagree with the live color, so the swatch
    // being painted with follows its own recolor.
    assert_eq!(state.style.quick_colors.color_for_index(3), Some(picked));
    assert_eq!(state.color_for_tool(Tool::Pen), picked);
    assert_eq!(state.style.current_color, picked);
    assert_eq!(state.preset_slots.active(), None);
    assert!(state.is_session_dirty());
}

#[test]
fn reopening_the_picker_abandons_an_unsaved_recolor() {
    let mut state = create_test_input_state();
    let first = state.style.quick_colors.color_for_index(0).expect("slot 0");
    let second = state.style.quick_colors.color_for_index(1).expect("slot 1");

    // Moving on to another swatch reverts the one left behind.
    assert!(state.open_color_picker_popup_for_quick_color(0));
    state.color_picker_popup_set_from_gradient(0.5, 0.5);
    assert!(state.open_color_picker_popup_for_quick_color(1));
    assert_eq!(state.style.quick_colors.color_for_index(0), Some(first));
    assert_eq!(state.color_picker_popup_slot(), Some(1));
    assert_eq!(state.color_picker_popup_current_color(), Some(second));

    // Right-clicking the same swatch again restarts from its saved color
    // rather than adopting the abandoned candidate as the new baseline.
    state.color_picker_popup_set_from_gradient(0.2, 0.7);
    assert!(state.open_color_picker_popup_for_quick_color(1));
    assert_eq!(state.style.quick_colors.color_for_index(1), Some(second));
    assert_eq!(state.color_picker_popup_current_color(), Some(second));

    // The tool chip does the same when it takes over the popup.
    state.color_picker_popup_set_from_gradient(0.9, 0.3);
    state.open_color_picker_popup();
    assert_eq!(state.style.quick_colors.color_for_index(1), Some(second));
    assert_eq!(state.color_picker_popup_slot(), None);
    assert!(state.active_toast().is_none());
}

#[test]
fn a_recolor_never_survives_an_implicit_close_unsaved() {
    let mut state = create_test_input_state();
    let slot_color = state.style.quick_colors.color_for_index(0).expect("slot 0");

    assert!(state.open_color_picker_popup_for_quick_color(0));
    state.color_picker_popup_set_from_gradient(0.5, 0.5);
    assert_ne!(
        state.style.quick_colors.color_for_index(0),
        Some(slot_color)
    );

    // Light mode and session restore close the popup without restoring; the
    // palette is durable config, so it still reverts.
    state.close_color_picker_popup(false);

    assert_eq!(
        state.style.quick_colors.color_for_index(0),
        Some(slot_color)
    );
    assert!(state.active_toast().is_none());
}

#[test]
fn recolor_hex_entry_lands_on_the_swatch() {
    let mut state = create_test_input_state();
    let tool_color = state.color_for_tool(Tool::Pen);

    assert!(state.open_color_picker_popup_for_quick_color(4));
    state.color_picker_popup_set_hex_editing(true);
    for ch in "1A2B3C".chars() {
        state.color_picker_popup_hex_append(ch);
    }
    assert!(state.color_picker_popup_commit_hex());

    let expected = crate::input::state::parse_hex_color("#1A2B3C").expect("hex");
    assert_eq!(state.style.quick_colors.color_for_index(4), Some(expected));
    assert_eq!(state.color_for_tool(Tool::Pen), tool_color);
}

#[test]
fn default_button_appears_only_for_slots_the_shipped_palette_defines() {
    let mut state = create_test_input_state();

    // The tool-color popup edits a value the palette does not own.
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let tool_layout = state.color_picker_popup_layout().expect("tool layout");
    assert!(!state.color_picker_popup_shows_default_button());
    assert!(tool_layout.default_btn.is_none());
    state.close_color_picker_popup(true);

    assert!(state.open_color_picker_popup_for_quick_color(1));
    state.update_color_picker_popup_layout(1920, 1080);
    let slot_layout = state.color_picker_popup_layout().expect("slot layout");
    let (default_x, default_y) = slot_layout.default_btn.expect("default button");
    assert_eq!(default_y, slot_layout.ok_btn_y, "shares the button row");
    // The row stays centered as a group, so the third button pushes OK/Cancel
    // right instead of overlapping them or leaving the panel.
    assert!(default_x + slot_layout.btn_width < slot_layout.ok_btn_x);
    assert!(slot_layout.ok_btn_x > tool_layout.ok_btn_x);
    assert!(default_x > slot_layout.origin_x);
    assert!(
        slot_layout.cancel_btn_x + slot_layout.btn_width < slot_layout.origin_x + slot_layout.width
    );
    assert!(slot_layout.point_in_default_button(default_x + 1.0, default_y + 1.0));
    assert!(!tool_layout.point_in_default_button(default_x + 1.0, default_y + 1.0));
    state.close_color_picker_popup(true);

    // A slot the user added past the built-in palette has no default to offer.
    state.set_quick_colors(crate::config::QuickColorPalette::from_entries(
        (0..13)
            .map(|index| crate::config::QuickColorPaletteEntry {
                label: format!("Custom {index}"),
                color: crate::draw::color::RED,
            })
            .collect(),
    ));
    assert!(state.open_color_picker_popup_for_quick_color(12));
    state.update_color_picker_popup_layout(1920, 1080);
    assert!(!state.color_picker_popup_shows_default_button());
    assert!(
        state
            .color_picker_popup
            .layout()
            .expect("layout")
            .default_btn
            .is_none()
    );
}

#[test]
fn default_button_stages_the_shipped_color_for_ok_to_accept() {
    let mut state = create_test_input_state();
    let shipped = crate::config::default_quick_color_for_index(1).expect("built-in slot 1");
    let customized = crate::draw::Color {
        r: 0.1,
        g: 0.2,
        b: 0.9,
        a: 1.0,
    };
    assert!(state.style.quick_colors.set_color_for_index(1, customized));

    assert!(state.open_color_picker_popup_for_quick_color(1));
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("layout");
    let (btn_x, btn_y) = layout.default_btn.expect("default button");
    let x = (btn_x + layout.btn_width / 2.0) as i32;
    let y = (btn_y + layout.btn_height / 2.0) as i32;

    assert!(state.handle_color_picker_press(MouseButton::Left, x, y));
    assert!(state.handle_color_picker_popup_release_at(x, y));

    // Restoring stages a candidate like any other pick: the swatch previews it
    // and the popup stays open so OK/Cancel still decide.
    assert!(state.is_color_picker_popup_open());
    assert_eq!(state.color_picker_popup_current_color(), Some(shipped));
    assert_eq!(state.style.quick_colors.color_for_index(1), Some(shipped));
    assert!(state.active_toast().is_none());

    state.apply_color_picker_popup();
    assert_eq!(state.style.quick_colors.color_for_index(1), Some(shipped));
    assert_eq!(
        state.take_pending_quick_color_edit(),
        Some(crate::input::state::QuickColorEdit {
            index: 1,
            color: shipped
        })
    );
}

#[test]
fn canceling_after_default_keeps_the_customized_color() {
    let mut state = create_test_input_state();
    let customized = crate::draw::Color {
        r: 0.4,
        g: 0.1,
        b: 0.3,
        a: 1.0,
    };
    assert!(state.style.quick_colors.set_color_for_index(2, customized));

    assert!(state.open_color_picker_popup_for_quick_color(2));
    assert!(state.color_picker_popup_restore_default());
    assert_ne!(
        state.style.quick_colors.color_for_index(2),
        Some(customized)
    );

    state.close_color_picker_popup(true);

    assert_eq!(
        state.style.quick_colors.color_for_index(2),
        Some(customized)
    );
    assert!(state.active_toast().is_none());
}

/// The OK button is clicked with the pointer, so the release path must reach
/// the same accept the keyboard path does.
#[test]
fn accepting_a_recolor_from_the_popup_release_queues_the_durable_write() {
    let mut state = create_test_input_state();

    assert!(state.open_color_picker_popup_for_quick_color(0));
    state.color_picker_popup_set_from_gradient(0.45, 0.35);
    let picked = state
        .color_picker_popup_current_color()
        .expect("picked color");
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("layout");
    let x = (layout.ok_btn_x + layout.btn_width / 2.0) as i32;
    let y = (layout.ok_btn_y + layout.btn_height / 2.0) as i32;

    assert!(state.handle_color_picker_press(MouseButton::Left, x, y));
    assert!(state.handle_color_picker_popup_release_at(x, y));

    assert!(!state.is_color_picker_popup_open());
    assert_eq!(state.style.quick_colors.color_for_index(0), Some(picked));
    assert_eq!(
        state.take_pending_quick_color_edit(),
        Some(crate::input::state::QuickColorEdit {
            index: 0,
            color: picked
        })
    );
}

#[test]
fn restoring_a_default_is_inert_without_a_slot() {
    let mut state = create_test_input_state();
    let tool_color = state.color_for_tool(Tool::Pen);

    state.open_color_picker_popup();

    assert!(!state.color_picker_popup_restore_default());
    assert_eq!(state.color_picker_popup_current_color(), Some(tool_color));
}

#[test]
fn recolor_popup_rejects_a_slot_past_the_palette() {
    let mut state = create_test_input_state();
    let len = state.style.quick_colors.len();

    assert!(!state.open_color_picker_popup_for_quick_color(len));
    assert!(!state.is_color_picker_popup_open());
}

#[test]
fn popup_title_names_the_slot_being_recolored() {
    let mut state = create_test_input_state();

    state.open_color_picker_popup();
    assert_eq!(state.color_picker_popup_title(), "Select Color");
    assert_eq!(state.color_picker_popup_slot(), None);

    let label = state
        .style
        .quick_colors
        .entry(1)
        .map(|entry| entry.label.clone())
        .expect("slot 1");
    assert!(state.open_color_picker_popup_for_quick_color(1));
    assert_eq!(state.color_picker_popup_title(), format!("Recolor {label}"));
}

#[test]
fn popup_title_flattens_and_bounds_an_authored_label() {
    let mut state = create_test_input_state();
    state.set_quick_colors(crate::config::QuickColorPalette::from_entries(vec![
        crate::config::QuickColorPaletteEntry {
            label: "  Ünreasonable\n\tlabel   with breaks  ".to_string(),
            color: crate::draw::color::RED,
        },
    ]));

    assert!(state.open_color_picker_popup_for_quick_color(0));

    // The title is one unwrapped line, so newlines, tabs and space runs are
    // flattened; a line break would otherwise draw outside the popup's damage.
    assert_eq!(
        state.color_picker_popup_title(),
        "Recolor Ünreasonable label with breaks"
    );

    // A pathological label is bounded so the renderer's width measurement never
    // walks a huge string. Fitting it to the panel stays the renderer's job.
    state.close_color_picker_popup(true);
    state.set_quick_colors(crate::config::QuickColorPalette::from_entries(vec![
        crate::config::QuickColorPaletteEntry {
            label: "W".repeat(5_000),
            color: crate::draw::color::RED,
        },
    ]));
    assert!(state.open_color_picker_popup_for_quick_color(0));
    assert!(state.color_picker_popup_title().chars().count() < 128);
}

#[test]
fn toolbar_edit_quick_color_event_opens_the_popup_on_that_slot() {
    let mut state = create_test_input_state();

    assert!(state.apply_toolbar_event(ToolbarEvent::EditQuickColor { index: 5 }));
    assert!(state.is_color_picker_popup_open());
    assert_eq!(state.color_picker_popup_slot(), Some(5));

    state.close_color_picker_popup(true);
    let stale = state.style.quick_colors.len() + 3;
    assert!(!state.apply_toolbar_event(ToolbarEvent::EditQuickColor { index: stale }));
    assert!(!state.is_color_picker_popup_open());
}

/// The square used to be hue-by-value with saturation pinned at 1.0, so no
/// pastel or muted colour was reachable by mouse and the rendered white top row
/// handed back a fully saturated colour.
#[test]
fn picker_square_reaches_desaturated_colors() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();

    // Left edge of the square is saturation 0: a pure grey ramp.
    state.color_picker_popup_set_from_gradient(0.0, 0.25);
    let color = state.color_picker_popup_current_color().expect("open");
    assert!(
        (color.r - color.g).abs() < 1e-6 && (color.g - color.b).abs() < 1e-6,
        "saturation 0 should give a grey, got {color:?}"
    );
    assert!(color.r > 0.7, "value 0.75 should be light, got {color:?}");
}

#[test]
fn picker_square_and_hue_bar_are_independent_axes() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();

    state.color_picker_popup_set_hue(0.5);
    state.color_picker_popup_set_from_gradient(1.0, 0.0);
    let cyan = state.color_picker_popup_current_color().expect("open");
    assert!(
        cyan.g > 0.9 && cyan.b > 0.9 && cyan.r < 0.1,
        "hue 0.5 at full saturation/value should be cyan, got {cyan:?}"
    );

    // Moving within the square must not disturb the hue.
    state.color_picker_popup_set_from_gradient(0.5, 0.5);
    let (hue, saturation, value) = state.color_picker_popup_hsv().expect("popup is open");
    assert!((hue - 0.5).abs() < 1e-6, "hue drifted to {hue}");
    assert!((saturation - 0.5).abs() < 1e-6);
    assert!((value - 0.5).abs() < 1e-6);
}

/// Grey, black and white all convert back to a hue of zero, so without the
/// remembered triple, dragging value to black and back would silently reset the
/// hue to red.
#[test]
fn picker_keeps_its_hue_through_black() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();

    state.color_picker_popup_set_hue(0.75);
    state.color_picker_popup_set_from_gradient(1.0, 1.0); // drag to black
    let color = state.color_picker_popup_current_color().expect("open");
    assert!(
        color.r < 1e-6 && color.g < 1e-6 && color.b < 1e-6,
        "expected black"
    );

    let hue = state.color_picker_popup_hue_position().expect("open");
    assert!(
        (hue - 0.75).abs() < 1e-6,
        "hue was lost through black: {hue}"
    );

    // Coming back up returns the original hue, not red.
    state.color_picker_popup_set_from_gradient(1.0, 0.0);
    let restored = state.color_picker_popup_current_color().expect("open");
    assert!(
        restored.b > 0.9 && restored.r > 0.4,
        "expected the violet back, got {restored:?}"
    );
}

#[test]
fn picker_square_position_round_trips() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();

    state.color_picker_popup_set_hue(0.33);
    state.color_picker_popup_set_from_gradient(0.4, 0.3);
    let (x, y) = state
        .color_picker_popup_gradient_position()
        .expect("popup is open");
    assert!((x - 0.4).abs() < 1e-6, "saturation round-trip lost: {x}");
    assert!((y - 0.3).abs() < 1e-6, "value round-trip lost: {y}");
}

/// A hand-edited session file must not be able to grow the list past its cap or
/// smuggle duplicates in. (The live dedupe/cap path is covered in the radial
/// menu tests; this covers the restore path, which is new.)
#[test]
fn restoring_recent_colors_reapplies_the_cap_and_dedupe() {
    let mut state = create_test_input_state();
    let red = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let mut oversized = vec![red, red];
    for value in 0..10u8 {
        oversized.push(Color {
            r: 0.0,
            g: f64::from(value) / 16.0,
            b: 0.0,
            a: 1.0,
        });
    }

    state.restore_recent_colors(oversized);

    let recents = state.recent_colors();
    assert_eq!(recents.len(), 6, "cap survives a hand-edited session");
    assert_eq!(recents[0], red);
    assert_eq!(
        recents.iter().filter(|c| **c == red).count(),
        1,
        "duplicates from the file are collapsed"
    );
}

#[test]
fn recent_swatches_are_hit_testable_in_the_popup() {
    let mut state = create_test_input_state();
    let stashed = Color {
        r: 0.25,
        g: 0.5,
        b: 0.875,
        a: 1.0,
    };
    state.apply_color_from_ui_with_measurer(&crate::draw::TextMeasurer::default(), stashed);
    state.apply_color_from_ui_with_measurer(
        &crate::draw::TextMeasurer::default(),
        Color {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    );

    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");

    let index = state
        .recent_colors()
        .iter()
        .position(|c| *c == stashed)
        .expect("stashed color is in recents");
    let (sx, sy) = layout.recent_swatch_origin(index);

    assert_eq!(
        layout.recent_swatch_at(sx + 2.0, sy + 2.0, state.recent_colors().len()),
        Some(index)
    );
}

#[test]
fn empty_recents_have_no_clickable_swatches() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let (sx, sy) = layout.recent_swatch_origin(0);

    assert_eq!(
        layout.recent_swatch_at(sx + 2.0, sy + 2.0, 0),
        None,
        "a fresh session shows no strip, so nothing is clickable"
    );

    // The cursor must agree: a hand over an invisible, inert slot promises a
    // click that does nothing.
    assert_eq!(
        layout.cursor_hint_at(sx + 2.0, sy + 2.0, 0),
        crate::input::state::ColorPickerCursorHint::Default
    );
    assert_eq!(
        layout.cursor_hint_at(sx + 2.0, sy + 2.0, 1),
        crate::input::state::ColorPickerCursorHint::Pointer,
        "a rendered swatch still gets the pointer"
    );
}

#[test]
fn picker_alpha_bar_sets_translucency_without_touching_hue() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.color_picker_popup_set_hue(0.5);
    state.color_picker_popup_set_from_gradient(1.0, 0.0);

    state.color_picker_popup_set_alpha(0.4);

    let color = state.color_picker_popup_current_color().expect("open");
    assert!((color.a - 0.4).abs() < 1e-9, "alpha not applied: {color:?}");
    assert!(
        color.g > 0.9 && color.b > 0.9 && color.r < 0.1,
        "hue changed"
    );
}

/// `hsv_to_rgb` is always opaque, so every move on the square or the hue bar
/// has to carry the alpha across or translucency silently vanishes.
#[test]
fn picker_keeps_alpha_across_square_and_hue_moves() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.color_picker_popup_set_alpha(0.25);

    state.color_picker_popup_set_from_gradient(0.6, 0.3);
    assert!(
        (state.color_picker_popup_current_color().expect("open").a - 0.25).abs() < 1e-9,
        "square drag reset the alpha"
    );

    state.color_picker_popup_set_hue(0.8);
    assert!(
        (state.color_picker_popup_current_color().expect("open").a - 0.25).abs() < 1e-9,
        "hue drag reset the alpha"
    );
}

/// A drag belongs to the control it started on, release included. Reading the
/// release position as a fresh click let a saturation drag that happened to
/// end over the alpha bar set alpha at its endpoint.
#[test]
fn a_picker_drag_ends_on_the_control_it_started_on() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.color_picker_popup_set_hue(0.5);
    state.color_picker_popup_set_alpha(0.6);
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");

    // Press near the left edge of the square, release over the far right of
    // the alpha bar (which as a fresh click would mean fully opaque).
    let press_x = (layout.sv_x + 4.0) as i32;
    let press_y = (layout.sv_y + layout.sv_h / 2.0) as i32;
    let release_x = (layout.alpha_x + layout.alpha_w - 2.0) as i32;
    let release_y = (layout.alpha_y + layout.alpha_h / 2.0) as i32;

    assert!(state.handle_color_picker_press(MouseButton::Left, press_x, press_y));
    assert!(state.handle_color_picker_popup_release_at(release_x, release_y));

    let color = state.color_picker_popup_current_color().expect("open");
    assert!(
        (color.a - 0.6).abs() < 1e-9,
        "releasing over the alpha bar hijacked the saturation drag: {color:?}"
    );
    let (hue, saturation, _) = state.color_picker_popup_hsv().expect("open");
    assert!((hue - 0.5).abs() < 1e-9, "hue moved: {hue}");
    assert!(
        saturation > 0.9,
        "the release position should still steer the square it started on, got {saturation}"
    );
}

#[test]
fn a_picker_drag_released_over_a_recent_swatch_does_not_adopt_it() {
    let mut state = create_test_input_state();
    let stashed = Color {
        r: 0.25,
        g: 0.5,
        b: 0.875,
        a: 1.0,
    };
    state.apply_color_from_ui_with_measurer(&crate::draw::TextMeasurer::default(), stashed);

    state.open_color_picker_popup();
    state.color_picker_popup_set_hue(0.1);
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let index = state
        .recent_colors()
        .iter()
        .position(|color| *color == stashed)
        .expect("stashed color is in recents");
    let (swatch_x, swatch_y) = layout.recent_swatch_origin(index);

    let press_x = (layout.hue_x + 4.0) as i32;
    let press_y = (layout.hue_y + layout.hue_h / 2.0) as i32;
    assert!(state.handle_color_picker_press(MouseButton::Left, press_x, press_y));
    assert!(
        state
            .handle_color_picker_popup_release_at((swatch_x + 2.0) as i32, (swatch_y + 2.0) as i32)
    );

    assert_ne!(
        state.color_picker_popup_current_color(),
        Some(stashed),
        "a hue drag released over a recent swatch adopted that swatch"
    );
}

/// The popup commits on its own edit target rather than through
/// `apply_color_from_ui`, so accepting has to record the color itself or a
/// color mixed in the picker never reaches the strip that shows recents.
#[test]
fn accepting_the_popup_records_the_color_in_recents() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.color_picker_popup_set_from_gradient(0.45, 0.35);
    let picked = state
        .color_picker_popup_current_color()
        .expect("picked color");

    state.apply_color_picker_popup();

    assert_eq!(state.recent_colors().first().copied(), Some(picked));

    // Cancelling stages nothing, so it records nothing.
    state.open_color_picker_popup();
    state.color_picker_popup_set_from_gradient(0.9, 0.1);
    state.close_color_picker_popup(true);

    assert_eq!(
        state.recent_colors().first().copied(),
        Some(picked),
        "a rejected color was recorded anyway"
    );

    // Recoloring a swatch the tool is not painting with edits the palette
    // without putting that color in use, so it is not a recent color either.
    let unused_slot = (0..state.style.quick_colors.len())
        .find(|index| state.style.quick_colors.color_for_index(*index) != Some(picked))
        .expect("a slot the tool is not using");
    assert!(state.open_color_picker_popup_for_quick_color(unused_slot));
    state.color_picker_popup_set_from_gradient(0.15, 0.65);
    state.apply_color_picker_popup();

    assert_eq!(
        state.recent_colors().first().copied(),
        Some(picked),
        "a palette edit was recorded as a color in use"
    );
}

#[test]
fn picker_hex_round_trips_alpha_only_when_translucent() {
    use crate::input::state::{HEX_INPUT_MAX_CHARS, color_to_hex, parse_hex_color};

    let opaque = Color {
        r: 1.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    };
    assert_eq!(color_to_hex(opaque), "#FF8000", "opaque stays six digits");

    let translucent = Color { a: 0.5, ..opaque };
    let hex = color_to_hex(translucent);
    assert_eq!(hex, "#FF800080");
    // Toolbar hex fields size and length-limit themselves by this constant, so
    // a wider form than they allow would be truncated or untypable.
    assert_eq!(hex.chars().count(), HEX_INPUT_MAX_CHARS);

    let parsed = parse_hex_color(&hex).expect("eight-digit hex parses");
    assert!((parsed.a - translucent.a).abs() < 0.01, "alpha round-trip");

    // Six-digit input still means fully opaque.
    assert!((parse_hex_color("#FF8000").expect("six digits").a - 1.0).abs() < 1e-9);
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
fn color_picker_action_buttons_expose_tooltips() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");

    let center = |x: f64, y: f64| {
        (
            x + layout.action_btn_size / 2.0,
            y + layout.action_btn_size / 2.0,
        )
    };
    let copy = center(layout.copy_btn_x, layout.copy_btn_y);
    let paste = center(layout.paste_btn_x, layout.paste_btn_y);
    let eyedropper = center(layout.eyedropper_btn_x, layout.eyedropper_btn_y);

    assert_eq!(
        layout.action_tooltip_at(copy.0, copy.1),
        Some("Copy hex color")
    );
    assert_eq!(
        layout.action_tooltip_at(paste.0, paste.1),
        Some("Paste hex color from clipboard")
    );
    assert_eq!(
        layout.action_tooltip_at(eyedropper.0, eyedropper.1),
        Some("Pick color from screen")
    );
    assert_eq!(
        layout.action_tooltip_at(layout.origin_x, layout.origin_y),
        None
    );
}

#[test]
fn color_picker_motion_tracks_action_hover_for_tooltips() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let x = (layout.copy_btn_x + layout.action_btn_size / 2.0) as i32;
    let y = (layout.copy_btn_y + layout.action_btn_size / 2.0) as i32;
    state.needs_redraw = false;

    state.on_mouse_motion_with_canvas(x, y, x, y);

    assert_eq!(state.color_picker_popup_hover(), Some((x as f64, y as f64)));
    assert!(state.needs_redraw);
}

#[test]
fn color_picker_motion_within_one_action_does_not_redraw_every_pixel() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let x = (layout.copy_btn_x + layout.action_btn_size / 2.0) as i32;
    let y = (layout.copy_btn_y + layout.action_btn_size / 2.0) as i32;

    state.on_mouse_motion_with_canvas(x, y, x, y);
    state.needs_redraw = false;
    state.on_mouse_motion_with_canvas(x + 1, y + 1, x + 1, y + 1);

    assert_eq!(
        state.color_picker_popup_hover(),
        Some(((x + 1) as f64, (y + 1) as f64))
    );
    assert!(!state.needs_redraw);
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
fn typing_a_hex_key_starts_manual_entry_without_a_click() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    assert!(!state.color_picker_popup_is_hex_editing());

    // A hex key focuses the field (armed to replace) and enters the char —
    // no click on the field, no select-all ceremony.
    state.on_key_press(Key::Char('a'));
    assert!(state.color_picker_popup_is_hex_editing());
    assert_eq!(state.color_picker_popup_hex_buffer(), Some("A"));

    // Continued typing builds the value and previews live once it parses.
    for ch in ['1', 'b', '2', 'c', '3'] {
        state.on_key_press(Key::Char(ch));
    }
    assert_eq!(state.color_picker_popup_hex_buffer(), Some("A1B2C3"));
    assert_eq!(
        state.color_picker_popup_current_color(),
        crate::input::state::parse_hex_color("A1B2C3")
    );
}

#[test]
fn hex_typing_defers_live_preview_until_six_digits() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    let original = state.color_picker_popup_current_color().unwrap();
    state.color_picker_popup_set_hex_editing(true);

    for ch in ['a', '1', 'b'] {
        state.on_key_press(Key::Char(ch));
    }
    assert_eq!(state.color_picker_popup_hex_buffer(), Some("A1B"));
    assert_eq!(state.color_picker_popup_current_color(), Some(original));

    for ch in ['2', 'c', '3'] {
        state.on_key_press(Key::Char(ch));
    }
    let expected = crate::input::state::parse_hex_color("A1B2C3").unwrap();
    assert_eq!(state.color_picker_popup_current_color(), Some(expected));
}

#[test]
fn three_digit_hex_still_applies_when_committed() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.color_picker_popup_set_hex_editing(true);
    for ch in ['a', '1', 'b'] {
        state.on_key_press(Key::Char(ch));
    }

    state.on_key_press(Key::Return);

    let expected = crate::input::state::parse_hex_color("A1B").unwrap();
    assert_eq!(state.color_picker_popup_current_color(), Some(expected));
    assert_eq!(state.color_picker_popup_hex_buffer(), Some("#AA11BB"));
    assert!(!state.color_picker_popup_is_hex_editing());
}

#[test]
fn color_picker_ok_button_commits_valid_three_digit_hex_before_apply() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    let original = state.color_for_tool(Tool::Pen);

    for ch in ['a', '1', 'b'] {
        state.on_key_press(Key::Char(ch));
    }
    assert_eq!(state.color_picker_popup_hex_buffer(), Some("A1B"));
    assert_eq!(state.color_picker_popup_current_color(), Some(original));

    state.update_color_picker_popup_layout(1920, 1080);
    let layout = state.color_picker_popup_layout().expect("popup layout");
    let x = (layout.ok_btn_x + layout.btn_width / 2.0) as i32;
    let y = (layout.ok_btn_y + layout.btn_height / 2.0) as i32;

    assert!(state.handle_color_picker_press(MouseButton::Left, x, y));
    assert!(state.handle_color_picker_popup_release_at(x, y));

    let expected = crate::input::state::parse_hex_color("A1B").unwrap();
    assert!(!state.is_color_picker_popup_open());
    assert_eq!(state.color_for_tool(Tool::Pen), expected);
    assert_eq!(state.style.current_color, expected);
}

#[test]
fn modified_hex_key_does_not_start_manual_entry() {
    let mut state = create_test_input_state();
    state.open_color_picker_popup();
    state.modifiers.ctrl = true;

    // Ctrl+C is a shortcut, not manual entry: it must not type into the field.
    state.on_key_press(Key::Char('c'));
    assert!(!state.color_picker_popup_is_hex_editing());
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
    assert_eq!(state.style.current_color, pen_color);

    state.color_picker_popup_set_from_gradient(0.3, 0.2);
    assert_ne!(state.color_for_tool(Tool::Line), line_color);
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.style.current_color, pen_color);

    state.close_color_picker_popup(true);
    assert_eq!(
        state.color_for_tool(Tool::Line),
        ColorSpec::from(line_color).to_color()
    );
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.style.current_color, pen_color);
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
    state.clear_session_dirty();

    state.on_key_press(Key::Shift);
    state.open_color_picker_popup();
    state.on_key_release(Key::Shift);

    state.color_picker_popup_set_from_gradient(0.3, 0.2);
    let applied = state.color_picker_popup_current_color().unwrap();
    state.apply_color_picker_popup();

    assert_eq!(state.color_for_tool(Tool::Line), applied);
    assert_eq!(state.color_for_tool(Tool::Pen), pen_color);
    assert_eq!(state.style.current_color, pen_color);
    assert!(state.is_session_dirty());
}

#[test]
fn save_preset_captures_all_tool_settings() {
    let mut state = create_test_input_state();
    state.preset_slots.set_slot_count_for_test(3);
    let styles = [
        (Tool::Pen, Color::new(0.1, 0.2, 0.3, 1.0), 4.0),
        (Tool::Line, Color::new(0.4, 0.5, 0.6, 1.0), 14.0),
        (Tool::Rect, Color::new(0.9, 0.1, 0.2, 1.0), 16.0),
        (Tool::Ellipse, Color::new(0.2, 0.9, 0.4, 1.0), 18.0),
        (Tool::Arrow, Color::new(0.8, 0.3, 0.7, 1.0), 20.0),
        (Tool::Blur, Color::new(0.3, 0.3, 0.3, 1.0), 22.0),
        (Tool::Marker, Color::new(0.7, 0.8, 0.1, 1.0), 24.0),
        (Tool::StepMarker, Color::new(0.2, 0.7, 0.9, 1.0), 30.0),
    ];

    for &(tool, color, size) in &styles {
        assert!(state.set_tool_override(Some(tool)));
        assert!(state.set_color(color));
        assert!(state.set_thickness(size));
    }
    assert!(state.set_tool_override(Some(Tool::Eraser)));
    assert!(state.set_eraser_size(33.0));
    assert!(state.set_tool_override(Some(Tool::Line)));

    assert!(state.save_preset(1));
    let preset = state.preset_slots.presets_mut_for_test()[0]
        .as_ref()
        .expect("saved preset");
    let tool_settings = preset.tool_settings.as_ref().expect("tool settings");

    assert_eq!(preset.tool, Tool::Line);
    assert_eq!(preset.color, ColorSpec::from(styles[1].1));
    assert_eq!(preset.size, 14.0);
    for (tool, color, size) in styles {
        assert_eq!(
            tool_settings.color_spec_for_tool(tool),
            ColorSpec::from(color)
        );
        assert_eq!(tool_settings.size_for_tool(tool), size);
    }
    assert_eq!(tool_settings.eraser_size, 33.0);
}

#[test]
fn save_preset_ignores_temporary_drag_modifier_tools() {
    let mut state = create_test_input_state();
    state.preset_slots.set_slot_count_for_test(4);
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
        let preset = state.preset_slots.presets_mut_for_test()[slot - 1]
            .as_ref()
            .expect("saved preset");
        assert_eq!(preset.tool, Tool::Pen);
        assert_eq!(preset.color, ColorSpec::from(pen_color));
        assert_eq!(preset.size, 7.0);
    }
}

#[test]
fn save_preset_without_override_uses_unmodified_drag_tool() {
    let mut state = create_test_input_state();
    state.preset_slots.set_slot_count_for_test(1);
    let marker_color = Color {
        r: 0.72,
        g: 0.18,
        b: 0.42,
        a: 1.0,
    };

    let mut bindings = state.drag_tool_bindings();
    bindings.left.drag = DragBinding::from_tool(Tool::Marker);
    assert!(state.set_drag_tool_bindings(bindings));
    assert!(state.set_tool_override(Some(Tool::Marker)));
    assert!(state.set_color(marker_color));
    assert!(state.set_thickness(19.0));
    assert!(state.set_tool_override(None));

    state.on_key_press(Key::Shift);
    assert_eq!(state.active_tool(), Tool::Line);
    assert!(state.save_preset(1));
    state.on_key_release(Key::Shift);

    let preset = state.preset_slots.presets_mut_for_test()[0]
        .as_ref()
        .expect("saved preset");
    assert_eq!(preset.tool, Tool::Marker);
    assert_eq!(preset.color, ColorSpec::from(marker_color));
    assert_eq!(preset.size, 19.0);
}

#[test]
fn apply_full_preset_restores_all_tool_settings() {
    let mut state = create_test_input_state();
    state.preset_slots.set_slot_count_for_test(3);
    let styles = [
        (Tool::Pen, Color::new(0.1, 0.2, 0.3, 1.0), 4.0),
        (Tool::Line, Color::new(0.4, 0.5, 0.6, 1.0), 14.0),
        (Tool::Rect, Color::new(0.9, 0.1, 0.2, 1.0), 16.0),
        (Tool::Ellipse, Color::new(0.2, 0.9, 0.4, 1.0), 18.0),
        (Tool::Arrow, Color::new(0.8, 0.3, 0.7, 1.0), 20.0),
        (Tool::Blur, Color::new(0.3, 0.3, 0.3, 1.0), 22.0),
        (Tool::Marker, Color::new(0.7, 0.8, 0.1, 1.0), 24.0),
        (Tool::StepMarker, Color::new(0.2, 0.7, 0.9, 1.0), 30.0),
    ];
    let mut settings = PerToolDrawingSettings::new(styles[0].1, styles[0].2);
    for &(tool, color, thickness) in &styles[1..] {
        let setting = settings.get_mut(tool);
        setting.color = color;
        setting.thickness = thickness;
    }
    let tool_settings = PresetToolStatesConfig::from_runtime(&settings, 33.0);

    state.preset_slots.presets_mut_for_test()[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Marker,
        color: ColorSpec::from(styles[6].1),
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
    for (tool, color, thickness) in styles {
        assert_eq!(
            state.color_for_tool(tool),
            ColorSpec::from(color).to_color()
        );
        assert_eq!(state.thickness_for_tool(tool), thickness);
    }
    assert_eq!(state.style.eraser_size, 33.0);
    assert_eq!(
        state.style.current_color,
        ColorSpec::from(styles[6].1).to_color()
    );
    assert_eq!(state.style.current_thickness, 24.0);
}

#[test]
fn toolbar_preset_preview_uses_nested_profile_for_active_preset_tool() {
    let mut state = create_test_input_state();
    state.preset_slots.set_slot_count_for_test(3);
    let top_level_color = ColorSpec::Rgb([255, 0, 0]);
    let pen_color = ColorSpec::Rgb([10, 20, 30]);
    let marker_color = ColorSpec::Rgb([200, 180, 20]);
    let mut settings = PerToolDrawingSettings::new(pen_color.to_color(), 3.0);
    settings.marker.color = marker_color.to_color();
    settings.marker.thickness = 22.0;
    let tool_settings = PresetToolStatesConfig::from_runtime(&settings, 18.0);

    state.preset_slots.presets_mut_for_test()[0] = Some(ToolPresetConfig {
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
    state.preset_slots.presets_mut_for_test()[1] = Some(ToolPresetConfig {
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
    state.preset_slots.set_slot_count_for_test(3);
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

    state.preset_slots.presets_mut_for_test()[0] = Some(ToolPresetConfig {
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
    state.preset_slots.set_slot_count_for_test(3);
    state.set_tool_override(Some(Tool::StepMarker));
    assert!(state.set_thickness(30.0));
    state.set_tool_override(Some(Tool::Pen));

    state.preset_slots.presets_mut_for_test()[0] = Some(ToolPresetConfig {
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
    assert_eq!(state.style.current_font_size, 48.0);
    assert!((state.thickness_for_tool(Tool::StepMarker) - 28.8).abs() < 1e-9);
    assert!((state.next_step_marker_label().size - 28.8).abs() < 1e-9);
}

#[test]
fn full_step_marker_preset_uses_captured_profile_size() {
    let mut state = create_test_input_state();
    state.preset_slots.set_slot_count_for_test(3);
    let color = ColorSpec::Rgb([20, 40, 60]);
    let mut settings = PerToolDrawingSettings::new(ColorSpec::Rgb([255, 0, 0]).to_color(), 4.0);
    settings.step_marker.color = color.to_color();
    settings.step_marker.thickness = 30.0;

    state.preset_slots.presets_mut_for_test()[0] = Some(ToolPresetConfig {
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
    assert_eq!(state.style.current_font_size, 48.0);
    assert_eq!(state.color_for_tool(Tool::StepMarker), color.to_color());
    assert_eq!(state.thickness_for_tool(Tool::StepMarker), 30.0);
    assert_eq!(state.next_step_marker_label().size, 30.0);
}

#[test]
fn nudge_thickness_for_active_tool_clamps_eraser_size() {
    let mut state = create_test_input_state();
    state.set_tool_override(Some(Tool::Eraser));
    state.style.eraser_size = 2.0;

    assert!(state.nudge_thickness_for_active_tool(-10.0));
    assert_eq!(state.style.eraser_size, 1.0);
}

#[test]
fn toggle_eraser_mode_round_trips_between_brush_and_stroke() {
    let mut state = create_test_input_state();
    assert_eq!(state.style.eraser_mode, EraserMode::Brush);

    assert!(state.toggle_eraser_mode());
    assert_eq!(state.style.eraser_mode, EraserMode::Stroke);

    assert!(state.toggle_eraser_mode());
    assert_eq!(state.style.eraser_mode, EraserMode::Brush);
}

#[test]
fn set_font_size_clamps_and_reports_noop_after_reaching_target() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.clear_session_dirty();

    assert!(state.set_font_size(120.0));
    assert_eq!(state.style.current_font_size, 72.0);
    assert!(state.needs_redraw);
    assert!(state.is_session_dirty());

    state.needs_redraw = false;
    state.clear_session_dirty();
    assert!(!state.set_font_size(72.0));
    assert!(!state.needs_redraw);
    assert!(!state.is_session_dirty());
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
    state.clear_session_dirty();

    assert!(state.set_font_descriptor(font.clone()));
    assert_eq!(state.style.font_descriptor, font);
    assert!(state.needs_redraw);
    assert!(state.is_session_dirty());

    state.needs_redraw = false;
    state.clear_session_dirty();
    assert!(!state.set_font_descriptor(font));
    assert!(!state.needs_redraw);
    assert!(!state.is_session_dirty());
}

#[test]
fn set_marker_opacity_clamps_and_reports_noop_after_reaching_target() {
    let mut state = create_test_input_state();
    state.needs_redraw = false;
    state.clear_session_dirty();

    assert!(state.set_marker_opacity(2.0));
    assert_eq!(state.style.marker_opacity, 0.9);
    assert!(state.needs_redraw);
    assert!(state.is_session_dirty());

    state.needs_redraw = false;
    state.clear_session_dirty();
    assert!(!state.set_marker_opacity(0.9));
    assert!(!state.needs_redraw);
    assert!(!state.is_session_dirty());
}

/// The write belongs to the backend, not to this layer.
///
/// Accepting a recolor updates the live palette and queues the slot; it must
/// not reach for the file itself. The durable half is covered where it lives,
/// beside `handle_quick_color_edit`.
#[test]
fn a_quick_color_recolor_queues_the_write_without_touching_the_file_itself() {
    crate::config::test_helpers::with_temp_config_home(|config_root| {
        let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
        std::fs::create_dir_all(&config_dir).expect("test config directory");
        let path = config_dir.join("config.toml");
        std::fs::write(
            &path,
            "[[drawing.quick_colors]]\nlabel = 'Authored'\ncolor = '#112233'\n",
        )
        .expect("test config should be written");
        let snapshot = crate::config::test_helpers::ConfigFileSnapshot::capture(&path);

        let configured = crate::config::Config::load()
            .expect("test config should load")
            .config;
        let authored = configured.drawing.quick_colors.effective_entries()[0]
            .color
            .clone();

        let mut state = create_test_input_state();
        state.set_quick_colors(crate::config::QuickColorPalette::from_config(
            &configured.drawing.quick_colors,
        ));
        assert!(state.open_color_picker_popup_for_quick_color(0));
        state.color_picker_popup_set_from_gradient(0.45, 0.35);
        let picked = state
            .color_picker_popup_current_color()
            .expect("picked color");
        state.apply_color_picker_popup();

        assert_eq!(state.style.quick_colors.color_for_index(0), Some(picked));
        assert_eq!(
            state.take_pending_quick_color_edit(),
            Some(crate::input::state::QuickColorEdit {
                index: 0,
                color: picked
            })
        );
        snapshot.assert_unchanged("accepting a quick-color recolor in InputState");

        let restarted = crate::config::Config::load()
            .expect("test config should reload")
            .config;
        assert_eq!(
            restarted.drawing.quick_colors.effective_entries()[0].color,
            authored,
            "nothing was written, because the backend has not drained the edit"
        );
    });
}

#[test]
fn cycling_arrow_style_with_nothing_selected_only_moves_the_next_arrow() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_test_input_state();
    let existing = state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 0,
        color: state.style.current_color,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Standard,
        bend: 0.0,
        label: None,
    });

    state.handle_action_with_resources(test_text_resources, Action::CycleArrowStyle);

    assert_eq!(state.style.arrow_style, ArrowStyle::Pointy);
    match &state
        .boards
        .active_frame()
        .shape(existing)
        .expect("arrow")
        .shape
    {
        Shape::Arrow { style, .. } => assert_eq!(
            *style,
            ArrowStyle::Standard,
            "an unselected arrow must not be restyled"
        ),
        other => panic!("expected arrow, got {other:?}"),
    }
}

#[test]
fn cycling_arrow_style_with_arrows_selected_restyles_them_instead() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_test_input_state();
    let arrow = state.boards.active_frame_mut().add_shape(Shape::Arrow {
        x1: 0,
        y1: 0,
        x2: 100,
        y2: 0,
        color: state.style.current_color,
        thick: 4.0,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        head_at_end: true,
        style: ArrowStyle::Standard,
        bend: 0.0,
        label: None,
    });
    state.set_selection(vec![arrow]);

    state.handle_action_with_resources(test_text_resources, Action::CycleArrowStyle);

    match &state
        .boards
        .active_frame()
        .shape(arrow)
        .expect("arrow")
        .shape
    {
        Shape::Arrow { style, .. } => assert_eq!(*style, ArrowStyle::Pointy),
        other => panic!("expected arrow, got {other:?}"),
    }
    assert_eq!(
        state.style.arrow_style,
        ArrowStyle::Standard,
        "restyling a selection must not also move the next-arrow default"
    );
}

#[test]
fn cycling_arrow_style_with_a_non_arrow_selected_falls_back_to_the_default() {
    let test_text_measurer = crate::draw::TextMeasurer::default();
    let test_ui_engine = crate::ui_text::UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_test_input_state();
    let rect = state.boards.active_frame_mut().add_shape(Shape::Rect {
        x: 0,
        y: 0,
        w: 20,
        h: 20,
        fill: false,
        color: state.style.current_color,
        thick: 2.0,
    });
    state.set_selection(vec![rect]);

    state.handle_action_with_resources(test_text_resources, Action::CycleArrowStyle);

    assert_eq!(state.style.arrow_style, ArrowStyle::Pointy);
}

#[test]
fn bold_reaches_selected_text_and_otherwise_sets_what_the_next_label_uses() {
    // The same target rule the font picker uses for a family: edit what the
    // user is looking at, or set the tool when they are looking at nothing.
    let mut state = create_test_input_state();
    let tool_weight = state.style.font_descriptor.weight.clone();
    let id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 10,
        y: 10,
        text: "hello".to_string(),
        color: crate::draw::Color::new(1.0, 1.0, 1.0, 1.0),
        size: 24.0,
        font_descriptor: crate::draw::FontDescriptor::new(
            "Sans".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
        background_enabled: false,
        wrap_width: None,
    });
    state.set_selection(vec![id]);

    assert!(state.apply_toolbar_event(ToolbarEvent::SetFontBold(true)));

    let frame = state.boards.active_frame();
    let Some(Shape::Text {
        font_descriptor, ..
    }) = frame.shape(id).map(|drawn| &drawn.shape)
    else {
        panic!("the text shape is still there");
    };
    assert!(font_descriptor.is_bold());
    assert_eq!(
        state.style.font_descriptor.weight, tool_weight,
        "restyling a selection must not also change what the next label uses"
    );

    // Nothing selected: the tool takes it instead. The built-in default weight
    // is already bold, so this starts by turning it off — which is the state a
    // user had no way back out of once the Sans/Mono segment was removed.
    state.clear_selection();
    assert!(state.apply_toolbar_event(ToolbarEvent::SetFontBold(false)));
    assert!(!state.style.font_descriptor.is_bold());
    assert!(state.apply_toolbar_event(ToolbarEvent::SetFontBold(true)));
    assert!(state.style.font_descriptor.is_bold());
}

#[test]
fn rendered_bold_control_reads_and_mutates_the_selected_text_target() {
    // Regression: the tool default is bold while the selected text is normal.
    // Building the shared rendered-control spec must produce an unchecked
    // toggle whose click bolds the selection, not a checked toggle whose click
    // sends the no-op "turn normal" event.
    let mut state = create_test_input_state();
    state.set_font_descriptor(crate::draw::FontDescriptor::new(
        "Sans".to_string(),
        "bold".to_string(),
        "normal".to_string(),
    ));
    let id = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 10,
        y: 10,
        text: "selected".to_string(),
        color: crate::draw::Color::new(1.0, 1.0, 1.0, 1.0),
        size: 24.0,
        font_descriptor: crate::draw::FontDescriptor::new(
            "Serif".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
        background_enabled: false,
        wrap_width: None,
    });
    state.set_selection(vec![id]);
    state.set_tool_override(Some(Tool::Select));

    let snapshot = ToolbarSnapshot::from_input(&state);
    let spec = StylePillSpec::build(&snapshot, &TopStripPlan::unconstrained());
    let bold = spec
        .controls()
        .iter()
        .copied()
        .find(|control| *control == StylePillControl::FontWeightToggle)
        .expect("selected text renders a Bold control");
    assert!(!bold.active(&snapshot));
    let event = bold.click_event(&snapshot);
    assert_eq!(event, ToolbarEvent::SetFontBold(true));

    assert!(state.apply_toolbar_event(event));
    let frame = state.boards.active_frame();
    let Some(Shape::Text {
        font_descriptor, ..
    }) = frame.shape(id).map(|drawn| &drawn.shape)
    else {
        panic!("the selected text is still there");
    };
    assert!(font_descriptor.is_bold());
    assert!(
        state.style.font_descriptor.is_bold(),
        "selected-text mutation leaves the tool default alone"
    );
}

#[test]
fn rendered_bold_control_skips_locked_text_and_disables_without_an_editable_target() {
    let mut state = create_test_input_state();
    let locked = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 10,
        y: 10,
        text: "locked bold".to_string(),
        color: crate::draw::Color::new(1.0, 1.0, 1.0, 1.0),
        size: 24.0,
        font_descriptor: crate::draw::FontDescriptor::new(
            "Sans".to_string(),
            "bold".to_string(),
            "normal".to_string(),
        ),
        background_enabled: false,
        wrap_width: None,
    });
    state
        .boards
        .active_frame_mut()
        .shape_mut(locked)
        .expect("locked text")
        .locked = true;
    let editable = state.boards.active_frame_mut().add_shape(Shape::Text {
        x: 20,
        y: 20,
        text: "editable normal".to_string(),
        color: crate::draw::Color::new(1.0, 1.0, 1.0, 1.0),
        size: 24.0,
        font_descriptor: crate::draw::FontDescriptor::new(
            "Sans".to_string(),
            "normal".to_string(),
            "normal".to_string(),
        ),
        background_enabled: false,
        wrap_width: None,
    });
    state.set_selection(vec![locked, editable]);
    state.set_tool_override(Some(Tool::Select));

    let snapshot = ToolbarSnapshot::from_input(&state);
    let spec = StylePillSpec::build(&snapshot, &TopStripPlan::unconstrained());
    let bold = spec
        .controls()
        .iter()
        .copied()
        .find(|control| *control == StylePillControl::FontWeightToggle)
        .expect("selected text renders a Bold control");
    assert!(bold.enabled(&snapshot));
    assert!(!bold.active(&snapshot), "editable normal text owns state");
    assert_eq!(bold.click_event(&snapshot), ToolbarEvent::SetFontBold(true));
    assert!(state.apply_toolbar_event(bold.click_event(&snapshot)));

    let frame = state.boards.active_frame();
    let is_bold = |id| match &frame.shape(id).expect("selected text").shape {
        Shape::Text {
            font_descriptor, ..
        } => font_descriptor.is_bold(),
        other => panic!("expected text, got {other:?}"),
    };
    assert!(is_bold(locked), "the locked bold shape stays bold");
    assert!(is_bold(editable), "the editable normal shape becomes bold");

    state
        .boards
        .active_frame_mut()
        .shape_mut(editable)
        .expect("editable text")
        .locked = true;
    let snapshot = ToolbarSnapshot::from_input(&state);
    assert!(snapshot.selection_has_text);
    assert_eq!(snapshot.selected_text_bold, None);
    assert!(!bold.enabled(&snapshot));
}

#[test]
fn turning_bold_off_leaves_the_family_and_style_alone() {
    let mut state = create_test_input_state();
    state.set_font_descriptor(crate::draw::FontDescriptor::new(
        "Serif".to_string(),
        "bold".to_string(),
        "italic".to_string(),
    ));

    assert!(state.apply_toolbar_event(ToolbarEvent::SetFontBold(false)));

    assert_eq!(state.style.font_descriptor.family, "Serif");
    assert_eq!(state.style.font_descriptor.style, "italic");
    assert!(!state.style.font_descriptor.is_bold());
}
