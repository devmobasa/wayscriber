use super::*;
use crate::config::{DragButtonConfig, MouseDragToolsConfig};
use crate::input::{DragBinding, DragTool, DragToolBindings};

#[test]
fn toggle_floating_badge_action_flips_runtime_visibility() {
    let mut state = create_test_input_state();
    assert!(state.show_floating_badge, "badge visible by default");

    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    assert!(!state.show_floating_badge);
    assert!(state.needs_redraw);
    // The badge preference is authored input: the toggle owns this run and
    // queues no durable work for the backend.
    assert!(!state.has_pending_backend_actions());
    assert!(state.take_pending_backend_action().is_none());

    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    assert!(state.show_floating_badge);
}

/// The chrome toggles are current-run changes, so the run says so once and
/// then stops repeating itself.
#[test]
fn chrome_visibility_toggles_announce_their_scope_once() {
    let mut state = create_test_input_state();

    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    let toast = state
        .ui_toast
        .as_ref()
        .expect("first toggle explains its scope");
    // The hint is the user's own binding for the configurator, not a
    // hard-coded key.
    assert_eq!(
        toast.message,
        "Applies to this run — edit defaults in the configurator (F11)."
    );
    state.ui_toast = None;

    state.handle_action(crate::config::Action::ToggleZoomChip);
    assert!(!state.show_zoom_chip);
    assert!(
        state.ui_toast.is_none(),
        "the scope is said once per run, not per toggle"
    );
}

/// With the configurator unbound there is no shortcut to name, so the notice
/// says where the default lives without inventing a key.
#[test]
fn process_only_notice_drops_the_hint_when_the_configurator_is_unbound() {
    let mut state = create_test_input_state();
    state.set_keybinding_maps(Default::default(), Default::default());

    state.handle_action(crate::config::Action::ToggleZoomChip);

    assert_eq!(
        state
            .ui_toast
            .as_ref()
            .map(|toast| toast.message.as_str())
            .unwrap_or_default(),
        "Applies to this run — edit defaults in the configurator."
    );
}

#[test]
fn repeated_chrome_visibility_toggles_stay_process_only() {
    let mut state = create_test_input_state();

    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    state.handle_action(crate::config::Action::ToggleFloatingBadge);
    state.handle_action(crate::config::Action::ToggleZoomChip);
    state.handle_action(crate::config::Action::ToggleZoomChip);

    assert!(state.show_floating_badge);
    assert!(state.show_zoom_chip);
    assert!(state.take_pending_backend_action().is_none());
}

#[test]
fn test_adjust_font_size_increase() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    state.adjust_font_size(2.0);
    assert_eq!(state.current_font_size, 34.0);
    assert!(state.needs_redraw);
}

#[test]
fn apply_preset_updates_tool_and_settings() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;
    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Marker,
        color: ColorSpec::Name("blue".to_string()),
        size: 12.0,
        tool_settings: None,
        eraser_kind: Some(EraserKind::Rect),
        eraser_mode: Some(EraserMode::Stroke),
        marker_opacity: Some(0.6),
        fill_enabled: Some(true),
        font_size: Some(28.0),
        text_background_enabled: Some(true),
        arrow_length: Some(25.0),
        arrow_angle: Some(45.0),
        arrow_head_at_end: Some(true),
        polygon_sides: Some(8),
        show_status_bar: Some(false),
        drag_tools: None,
    });

    assert!(state.apply_preset(1));
    assert_eq!(state.active_tool(), Tool::Marker);
    assert_eq!(
        state.current_color,
        ColorSpec::Name("blue".to_string()).to_color()
    );
    assert_eq!(state.current_thickness, 12.0);
    assert_eq!(state.marker_opacity, 0.6);
    assert!(state.fill_enabled);
    assert_eq!(state.current_font_size, 28.0);
    assert!(state.text_background_enabled);
    assert_eq!(state.arrow_length, 25.0);
    assert_eq!(state.arrow_angle, 45.0);
    assert!(state.arrow_head_at_end);
    assert_eq!(state.polygon_sides, 8);
    assert_eq!(state.eraser_kind, EraserKind::Rect);
    assert_eq!(state.eraser_mode, EraserMode::Stroke);
    assert!(!state.show_status_bar);
}

#[test]
fn apply_preset_merges_partial_left_drag_tool_bindings() {
    let mut state = create_test_input_state();
    state.preset_slot_count = 3;

    let mut existing_bindings = DragToolBindings::default();
    existing_bindings.left.shift_drag = DragBinding::from_tool(Tool::Eraser);
    assert!(state.set_drag_tool_bindings(existing_bindings));

    let mut left = DragButtonConfig::button_behavior();
    left.drag_tool = DragTool::Marker;

    state.presets[0] = Some(ToolPresetConfig {
        name: None,
        tool: Tool::Marker,
        color: ColorSpec::Name("blue".to_string()),
        size: 12.0,
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
        drag_tools: Some(MouseDragToolsConfig::from_buttons(
            left,
            DragButtonConfig::button_behavior(),
            DragButtonConfig::button_behavior(),
        )),
    });

    assert!(state.apply_preset(1));
    assert_eq!(state.drag_tool_bindings.left.drag.tool, DragTool::Marker);
    assert_eq!(
        state.drag_tool_bindings.left.shift_drag.tool,
        DragTool::Eraser
    );
    assert_eq!(state.drag_tool_bindings.left.ctrl_drag.tool, DragTool::Rect);
    assert_eq!(
        state.drag_tool_bindings.left.ctrl_shift_drag.tool,
        DragTool::Arrow
    );
    assert_eq!(
        state.drag_tool_bindings.left.tab_drag.tool,
        DragTool::Ellipse
    );
}

#[test]
fn test_adjust_font_size_decrease() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    state.adjust_font_size(-2.0);
    assert_eq!(state.current_font_size, 30.0);
    assert!(state.needs_redraw);
}

#[test]
fn test_toggle_all_highlights_toggles_both() {
    let mut state = create_test_input_state();

    // Start off disabled
    assert!(!state.highlight_tool_active());
    assert!(!state.click_highlight_enabled());

    // Enable: should turn on both tool and click highlight
    let enabled = state.toggle_all_highlights();
    assert!(enabled);
    assert!(state.highlight_tool_active());
    assert!(state.click_highlight_enabled());

    // Disable: should turn off both
    let enabled_after = state.toggle_all_highlights();
    assert!(!enabled_after);
    assert!(!state.highlight_tool_active());
    assert!(!state.click_highlight_enabled());
}

#[test]
fn test_adjust_font_size_clamp_min() {
    let mut state = create_test_input_state();
    state.current_font_size = 10.0;

    // Try to go below minimum (8.0)
    state.adjust_font_size(-5.0);
    assert_eq!(state.current_font_size, 8.0);
}

#[test]
fn test_adjust_font_size_clamp_max() {
    let mut state = create_test_input_state();
    state.current_font_size = 70.0;

    // Try to go above maximum (72.0)
    state.adjust_font_size(5.0);
    assert_eq!(state.current_font_size, 72.0);
}

#[test]
fn test_adjust_font_size_at_boundaries() {
    let mut state = create_test_input_state();

    // Test at minimum boundary
    state.current_font_size = 8.0;
    state.adjust_font_size(0.0);
    assert_eq!(state.current_font_size, 8.0);

    // Test at maximum boundary
    state.current_font_size = 72.0;
    state.adjust_font_size(0.0);
    assert_eq!(state.current_font_size, 72.0);
}

#[test]
fn test_adjust_font_size_multiple_adjustments() {
    let mut state = create_test_input_state();
    assert_eq!(state.current_font_size, 32.0);

    // Simulate multiple Ctrl+Shift++ presses
    state.adjust_font_size(2.0);
    state.adjust_font_size(2.0);
    state.adjust_font_size(2.0);
    assert_eq!(state.current_font_size, 38.0);

    // Then decrease
    state.adjust_font_size(-2.0);
    state.adjust_font_size(-2.0);
    assert_eq!(state.current_font_size, 34.0);
}

#[test]
fn toolbar_toggle_handles_partial_visibility() {
    let mut state = create_test_input_state();
    // Partial visibility is a side-palette scenario: opt into the
    // deprecated Panel escape hatch (the struct default is Pill).
    state.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Panel);
    // Simulate config: top pinned, side not pinned
    state.init_toolbar_from_config(
        crate::config::ToolbarLayoutMode::Regular,
        crate::config::ToolbarModeOverrides::default(),
        crate::config::ToolbarItemsConfig::default(),
        true,  // top_pinned
        false, // side_pinned
        true,  // use_icons
        1.0,   // scale
        false, // show_more_colors
        true,  // show_actions_section
        false, // show_actions_advanced
        true,  // show_zoom_actions
        true,  // show_pages_section
        true,  // show_boards_section
        true,  // show_presets
        false, // show_step_section
        false, // show_text_controls
        true,  // context_aware_ui
        true,  // show_settings_section
        false, // show_delay_sliders
        false, // show_marker_opacity_section
        true,  // show_preset_toasts
        false, // show_tool_preview
    );
    assert!(state.toolbar_top_visible());
    assert!(!state.toolbar_side_visible());
    assert!(state.toolbar_visible());

    // Toggle off
    let _ = state.set_toolbar_visible(!state.toolbar_visible());
    assert!(!state.toolbar_visible());
    assert!(!state.toolbar_top_visible());
    assert!(!state.toolbar_side_visible());

    // Toggle on
    let _ = state.set_toolbar_visible(!state.toolbar_visible());
    assert!(state.toolbar_visible());
    assert!(state.toolbar_top_visible());
    assert!(state.toolbar_side_visible());
}
