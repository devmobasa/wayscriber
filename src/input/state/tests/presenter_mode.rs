use super::create_test_input_state;
use crate::config::{ColorSpec, ToolPresetConfig};
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

#[test]
fn presenter_mode_forces_click_highlight() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.enable_click_highlight = true;

    assert!(!state.click_highlight_enabled());
    state.toggle_presenter_mode();
    assert!(state.presenter_mode);
    assert!(state.click_highlight_enabled());

    state.toggle_all_highlights();
    assert!(state.click_highlight_enabled());
}

#[test]
fn presenter_mode_blocks_preset_status_bar_toggle() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_status_bar = true;

    let preset = ToolPresetConfig {
        name: None,
        tool: Tool::Pen,
        color: ColorSpec::Name("red".to_string()),
        size: 5.0,
        eraser_kind: None,
        eraser_mode: None,
        marker_opacity: None,
        fill_enabled: None,
        font_size: None,
        text_background_enabled: None,
        arrow_length: None,
        arrow_angle: None,
        arrow_head_at_end: None,
        show_status_bar: Some(true),
    };
    state.presets[0] = Some(preset);

    state.toggle_presenter_mode();
    assert!(!state.show_status_bar);

    assert!(state.apply_preset(1));
    assert!(!state.show_status_bar);
}

#[test]
fn presenter_mode_blocks_tool_preview_toggle() {
    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_tool_preview = true;

    state.toggle_presenter_mode();
    assert!(!state.show_tool_preview);

    assert!(!state.apply_toolbar_event(ToolbarEvent::ToggleToolPreview(true)));
    assert!(!state.show_tool_preview);
}
