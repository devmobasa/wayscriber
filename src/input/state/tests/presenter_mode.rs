use super::create_test_input_state;
use crate::config::{ColorSpec, PresenterToolBehavior, ToolPresetConfig};
use crate::input::{DragBinding, DragToolBindings, MouseButton, Tool};
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
        show_status_bar: Some(true),
        drag_tools: None,
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

#[test]
fn presenter_mode_closes_help_overlay_and_switches_to_highlight_tool() {
    let mut state = create_test_input_state();
    state.show_help = true;
    state.set_tool_override(Some(Tool::Pen));

    state.toggle_presenter_mode();

    assert!(state.presenter_mode);
    assert!(!state.show_help);
    assert_eq!(state.tool_override(), Some(Tool::Highlight));
}

#[test]
fn presenter_locked_mode_blocks_non_left_drag_bindings() {
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::from_tool(Tool::Pen);
    assert!(state.set_drag_tool_bindings(bindings));
    state.presenter_mode_config.tool_behavior = PresenterToolBehavior::ForceHighlightLocked;

    state.toggle_presenter_mode();
    state.on_mouse_press(MouseButton::Right, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Right, 10, 10);

    assert!(state.boards.active_frame().shapes.is_empty());
    assert_eq!(state.tool_override(), Some(Tool::Highlight));
}

#[test]
fn presenter_mode_restores_status_bar_toolbars_and_tool_override_on_exit() {
    let mut state = create_test_input_state();
    state.show_status_bar = true;
    state.toolbar_visible = true;
    state.toolbar_top_visible = true;
    state.toolbar_side_visible = true;
    state.set_tool_override(Some(Tool::Arrow));

    state.toggle_presenter_mode();
    assert!(!state.show_status_bar);
    assert!(!state.toolbar_visible);
    assert_eq!(state.tool_override(), Some(Tool::Highlight));

    state.toggle_presenter_mode();
    assert!(!state.presenter_mode);
    assert!(state.show_status_bar);
    assert!(state.toolbar_visible);
    assert!(state.toolbar_top_visible);
    assert!(state.toolbar_side_visible);
    assert_eq!(state.tool_override(), Some(Tool::Arrow));
}

#[test]
fn presenter_micro_mapping_shows_the_chip_and_restores_on_exit() {
    use crate::config::{PresenterToolbarMode, TopDisplayMode};

    let mut state = create_test_input_state();
    // Side-palette assertions below need the deprecated Panel escape hatch
    // (the struct default is Pill, which retires the side surface).
    state.init_toolbar_side_layout_from_config(crate::config::ToolbarSideLayout::Panel);
    state.presenter_mode_config.hide_toolbars = true;
    state.presenter_mode_config.toolbar_mode = PresenterToolbarMode::Micro;
    state.toolbar_visible = true;
    state.toolbar_top_visible = true;
    state.toolbar_side_visible = true;
    state.toolbar_top_minimized = true;

    state.toggle_presenter_mode();
    assert!(state.presenter_mode);
    assert!(
        state.toolbar_top_visible(),
        "micro mapping keeps the top strip surface mapped"
    );
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);
    assert!(
        !state.toolbar_top_minimized,
        "the chip replaces the restore tab"
    );
    assert!(!state.toolbar_side_visible(), "side toolbars still hide");

    state.toggle_presenter_mode();
    assert!(!state.presenter_mode);
    assert_eq!(state.toolbar_top_display_mode, TopDisplayMode::Full);
    assert!(
        state.toolbar_top_minimized,
        "minimize state restored on exit"
    );
    assert!(state.toolbar_side_visible());
}

#[test]
fn presenter_hidden_mapping_keeps_todays_behavior() {
    use crate::config::{PresenterToolbarMode, TopDisplayMode};

    let mut state = create_test_input_state();
    state.presenter_mode_config.hide_toolbars = true;
    state.presenter_mode_config.toolbar_mode = PresenterToolbarMode::Hidden;

    state.toggle_presenter_mode();
    assert!(!state.toolbar_top_visible());
    assert!(!state.toolbar_side_visible());
    assert_eq!(state.toolbar_top_display_mode, TopDisplayMode::Full);
}

#[test]
fn presenter_mode_emits_entry_and_exit_toasts() {
    let mut state = create_test_input_state();

    state.toggle_presenter_mode();
    let entry_toast = state.ui_toast.as_ref().expect("entry toast");
    assert_eq!(entry_toast.message, "Presenter Mode active");
    assert_eq!(
        entry_toast.action.as_ref().map(|action| action.action),
        Some(crate::config::Action::TogglePresenterMode)
    );

    state.toggle_presenter_mode();
    let exit_toast = state.ui_toast.as_ref().expect("exit toast");
    assert_eq!(exit_toast.message, "Stopping Presenter Mode");
    assert!(exit_toast.action.is_none());
}
