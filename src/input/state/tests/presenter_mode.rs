use super::create_test_input_state;
use crate::config::{
    Action, ColorSpec, PresenterToolBehavior, PresenterToolbarMode, ToolPresetConfig,
    TopDisplayMode,
};
use crate::input::{DragBinding, DragToolBindings, MouseButton, Tool};
use crate::ui::toolbar::ToolbarEvent;

#[test]
fn presenter_mode_forces_click_highlight() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    let mut state = create_test_input_state();
    state
        .presenter_mode_config_mut_for_test()
        .enable_click_highlight = true;

    assert!(!state.click_highlight_enabled());
    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(state.presenter_mode_active());
    assert!(state.click_highlight_enabled());

    state.toggle_all_highlights();
    assert!(state.click_highlight_enabled());
}

#[test]
fn presenter_mode_exits_focus_mode_before_taking_chrome_ownership() {
    let mut state = create_test_input_state();
    state.presenter_mode_config_mut_for_test().hide_status_bar = true;
    state.presenter_mode_config_mut_for_test().hide_toolbars = true;
    state.presenter_mode_config_mut_for_test().toolbar_mode = PresenterToolbarMode::Micro;

    state.handle_action(Action::ToggleFocusMode);
    assert!(state.focus_mode_active());
    assert!(!state.ui_visibility.show_status_bar);

    state.handle_action(Action::TogglePresenterMode);

    assert!(state.presenter_mode_active());
    assert!(
        !state.focus_mode_active(),
        "Presenter Mode must become the sole chrome snapshot owner"
    );
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);

    state.handle_action(Action::TogglePresenterMode);
    assert!(!state.presenter_mode_active());
    assert!(
        state.ui_visibility.show_status_bar,
        "pre-Focus visibility must survive"
    );
    assert!(state.toolbar_visible(), "pre-Focus toolbar must survive");
}

#[test]
fn presenter_mode_blocks_preset_status_bar_toggle() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    let mut state = create_test_input_state();
    state.presenter_mode_config_mut_for_test().hide_status_bar = true;

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
    state.preset_slots.presets_mut_for_test()[0] = Some(preset);

    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(!state.ui_visibility.show_status_bar);

    assert!(state.apply_preset(1));
    assert!(!state.ui_visibility.show_status_bar);
}

#[test]
fn presenter_mode_blocks_tool_preview_toggle() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    let mut state = create_test_input_state();
    state.presenter_mode_config_mut_for_test().hide_tool_preview = true;

    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(!state.ui_visibility.show_tool_preview);

    assert!(!state.apply_toolbar_event(ToolbarEvent::ToggleToolPreview(true)));
    assert!(!state.ui_visibility.show_tool_preview);
}

#[test]
fn presenter_mode_closes_help_overlay_and_switches_to_highlight_tool() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    let mut state = create_test_input_state();
    state.help_overlay.visible = true;
    state.set_tool_override(Some(Tool::Pen));

    state.toggle_presenter_mode_with_resources(route_resources);

    assert!(state.presenter_mode_active());
    assert!(!state.help_overlay.visible);
    assert_eq!(state.tool_override(), Some(Tool::Highlight));
}

#[test]
fn presenter_locked_mode_blocks_non_left_drag_bindings() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    let mut state = create_test_input_state();
    let mut bindings = DragToolBindings::default();
    bindings.right.drag = DragBinding::from_tool(Tool::Pen);
    assert!(state.set_drag_tool_bindings(bindings));
    state.presenter_mode_config_mut_for_test().tool_behavior =
        PresenterToolBehavior::ForceHighlightLocked;

    state.toggle_presenter_mode_with_resources(route_resources);
    state.on_mouse_press(MouseButton::Right, 0, 0);
    state.on_mouse_motion(10, 10);
    state.on_mouse_release(MouseButton::Right, 10, 10);

    assert!(state.boards.active_frame().shapes.is_empty());
    assert_eq!(state.tool_override(), Some(Tool::Highlight));
}

#[test]
fn presenter_mode_restores_status_bar_toolbars_and_tool_override_on_exit() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    let mut state = create_test_input_state();
    state.ui_visibility.show_status_bar = true;
    state.test_set_toolbar_visibility_state(true, true, state.toolbar_top_pinned());
    state.set_tool_override(Some(Tool::Arrow));

    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(!state.ui_visibility.show_status_bar);
    assert!(!state.toolbar_visible());
    assert_eq!(state.tool_override(), Some(Tool::Highlight));

    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(!state.presenter_mode_active());
    assert!(state.ui_visibility.show_status_bar);
    assert!(state.toolbar_visible());
    assert!(state.toolbar_top_visible());
    assert_eq!(state.tool_override(), Some(Tool::Arrow));
}

#[test]
fn presenter_micro_mapping_shows_the_chip_and_restores_on_exit() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    use crate::config::{PresenterToolbarMode, TopDisplayMode};

    let mut state = create_test_input_state();
    state.presenter_mode_config_mut_for_test().hide_toolbars = true;
    state.presenter_mode_config_mut_for_test().toolbar_mode = PresenterToolbarMode::Micro;
    state.test_set_toolbar_visibility_state(true, true, state.toolbar_top_pinned());
    state.test_set_toolbar_display_state(state.toolbar_top_display_mode(), true);

    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(state.presenter_mode_active());
    assert!(
        state.toolbar_top_visible(),
        "micro mapping keeps the top strip surface mapped"
    );
    assert_eq!(state.top_display_state(), TopDisplayMode::Micro);
    assert!(
        !state.toolbar_top_minimized(),
        "the chip replaces the restore tab"
    );
    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(!state.presenter_mode_active());
    assert_eq!(state.toolbar_top_display_mode(), TopDisplayMode::Full);
    assert!(
        state.toolbar_top_minimized(),
        "minimize state restored on exit"
    );
}

#[test]
fn presenter_hidden_mapping_keeps_todays_behavior() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    use crate::config::{PresenterToolbarMode, TopDisplayMode};

    let mut state = create_test_input_state();
    state.presenter_mode_config_mut_for_test().hide_toolbars = true;
    state.presenter_mode_config_mut_for_test().toolbar_mode = PresenterToolbarMode::Hidden;

    state.toggle_presenter_mode_with_resources(route_resources);
    assert!(!state.toolbar_top_visible());
    assert_eq!(state.toolbar_top_display_mode(), TopDisplayMode::Full);
}

#[test]
fn presenter_mode_emits_entry_and_exit_toasts() {
    let route_measurer = crate::draw::TextMeasurer::default();
    let route_ui_engine = crate::ui_text::UiTextEngine::default();
    let route_resources = crate::input::state::InputTextResources {
        measurer: &route_measurer,
        ui_engine: &route_ui_engine,
    };
    let mut state = create_test_input_state();

    state.toggle_presenter_mode_with_resources(route_resources);
    let entry_toast = state.active_toast().expect("entry toast");
    assert_eq!(entry_toast.message, "Presenter Mode active");
    assert_eq!(
        entry_toast
            .action
            .as_ref()
            .and_then(|action| action.dispatch_action()),
        Some(crate::config::Action::TogglePresenterMode)
    );

    state.toggle_presenter_mode_with_resources(route_resources);
    let exit_toast = state.active_toast().expect("exit toast");
    assert_eq!(exit_toast.message, "Stopping Presenter Mode");
    assert!(exit_toast.action.is_none());
}
