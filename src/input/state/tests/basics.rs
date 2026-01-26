use super::*;

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
        eraser_kind: Some(EraserKind::Rect),
        eraser_mode: Some(EraserMode::Stroke),
        marker_opacity: Some(0.6),
        fill_enabled: Some(true),
        font_size: Some(28.0),
        text_background_enabled: Some(true),
        arrow_length: Some(25.0),
        arrow_angle: Some(45.0),
        arrow_head_at_end: Some(true),
        show_status_bar: Some(false),
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
    assert_eq!(state.eraser_kind, EraserKind::Rect);
    assert_eq!(state.eraser_mode, EraserMode::Stroke);
    assert!(!state.show_status_bar);
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
    // Simulate config: top pinned, side not pinned
    state.init_toolbar_from_config(
        crate::config::ToolbarLayoutMode::Regular,
        crate::config::ToolbarModeOverrides::default(),
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
