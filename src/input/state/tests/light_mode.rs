use crate::draw::TextMeasurer;
use crate::input::InputState;
use crate::ui_text::UiTextEngine;

use super::{create_test_input_state, create_test_input_state_with_click_highlight};
use crate::config::{Action, Config};
use crate::input::{ClickHighlightSettings, Tool};

fn create_light_mode_test_state() -> InputState {
    let mut state = create_test_input_state();
    state.compositor_capabilities.layer_shell = true;
    state
}

fn create_light_mode_test_state_with_click_highlight(
    click_highlight_settings: ClickHighlightSettings,
) -> InputState {
    let mut state = create_test_input_state_with_click_highlight(click_highlight_settings);
    state.compositor_capabilities.layer_shell = true;
    state
}

#[test]
fn light_mode_enters_passthrough_and_hides_heavy_ui() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_light_mode_test_state();
    state.ui_visibility.show_status_bar = true;
    state.test_set_toolbar_visibility_state(true, true, state.toolbar_top_pinned());
    state.ui_visibility.show_tool_preview = true;
    state.set_tool_override(Some(Tool::Arrow));

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(state.light_mode_active());
    assert!(!state.light_mode_drawing_active());
    assert!(state.light_mode_passthrough());
    assert!(!state.ui_visibility.show_status_bar);
    assert!(!state.toolbar_visible());
    assert!(!state.toolbar_top_visible());
    assert!(!state.ui_visibility.show_tool_preview);
    assert_eq!(state.tool_override(), Some(Tool::Pen));
}

#[test]
fn light_mode_keeps_an_active_laser_instead_of_switching_to_the_pen() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_light_mode_test_state();
    state.set_tool_override(Some(Tool::Laser));

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);
    assert!(state.light_mode_active());
    assert_eq!(state.tool_override(), Some(Tool::Laser));

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);
    assert_eq!(state.tool_override(), Some(Tool::Laser));
}

#[test]
fn light_mode_drawing_toggle_disables_passthrough_without_exiting() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_light_mode_test_state();

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);
    state.handle_action_with_resources(test_text_resources, Action::ToggleLightModeDrawing);

    assert!(state.light_mode_active());
    assert!(state.light_mode_drawing_active());
    assert!(!state.light_mode_passthrough());

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightModeDrawing);

    assert!(state.light_mode_active());
    assert!(!state.light_mode_drawing_active());
    assert!(state.light_mode_passthrough());
}

#[test]
fn light_draw_off_does_not_enter_light_mode() {
    let mut state = create_light_mode_test_state();

    assert!(!state.set_light_mode_drawing(false));

    assert!(!state.light_mode_active());
    assert!(!state.light_mode_drawing_active());
}

#[test]
fn light_mode_restores_previous_ui_and_tool_on_exit() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_light_mode_test_state();
    state.ui_visibility.show_status_bar = true;
    state.test_set_toolbar_visibility_state(true, false, state.toolbar_top_pinned());
    state.ui_visibility.show_tool_preview = true;
    state.set_tool_override(Some(Tool::Marker));

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);
    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(!state.light_mode_active());
    assert!(!state.light_mode_drawing_active());
    assert!(!state.light_mode_passthrough());
    assert!(state.ui_visibility.show_status_bar);
    assert!(state.toolbar_visible_flag());
    assert!(!state.toolbar_top_visible_flag());
    assert!(state.ui_visibility.show_tool_preview);
    assert_eq!(state.tool_override(), Some(Tool::Marker));
}

#[test]
fn light_mode_force_enables_click_highlight_by_default_and_restores_on_exit() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_light_mode_test_state();
    assert!(!state.click_highlight_enabled());

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(state.light_mode_active());
    assert!(state.click_highlight_enabled());

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(!state.light_mode_active());
    assert!(!state.click_highlight_enabled());
}

#[test]
fn light_mode_can_leave_click_highlight_disabled() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut settings = ClickHighlightSettings::disabled();
    settings.force_in_light_mode = false;
    let mut state = create_light_mode_test_state_with_click_highlight(settings);

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(state.light_mode_active());
    assert!(!state.click_highlight_enabled());

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightModeDrawing);

    assert!(state.light_mode_drawing_active());
    assert!(!state.click_highlight_enabled());
}

#[test]
fn light_mode_and_presenter_mode_are_mutually_exclusive() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_light_mode_test_state();

    state.handle_action_with_resources(test_text_resources, Action::TogglePresenterMode);
    assert!(state.presenter_mode_active());

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(state.light_mode_active());
    assert!(!state.presenter_mode_active());
}

#[test]
fn light_mode_does_not_enter_without_layer_shell() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };

    let mut state = create_test_input_state();

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(!state.light_mode_active());
    assert!(!state.light_mode_passthrough());
}

#[test]
fn light_mode_transitions_show_toasts_by_default() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };
    let mut state = create_light_mode_test_state();

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert_eq!(
        state.test_active_toast_message(),
        Some("Light Mode passthrough")
    );
    assert_eq!(
        state
            .active_toast()
            .and_then(|toast| toast.action.as_ref())
            .and_then(|action| action.dispatch_action()),
        Some(Action::ToggleLightMode),
        "the entry toast offers an Exit action"
    );

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightModeDrawing);

    assert_eq!(
        state.test_active_toast_message(),
        Some("Light Mode drawing")
    );

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert_eq!(
        state.test_active_toast_message(),
        Some("Stopping Light Mode")
    );
}

#[test]
fn light_mode_transitions_stay_silent_when_toasts_are_disabled() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };
    let config: Config = toml::from_str("[ui]\nshow_light_mode_toasts = false\n")
        .expect("light mode toast preference should parse");
    let mut state = InputState::from_config(&config);
    state.compositor_capabilities.layer_shell = true;

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(state.light_mode_passthrough());
    assert!(state.toasts_idle(), "entering light mode must not toast");

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightModeDrawing);

    assert!(state.light_mode_drawing_active());
    assert!(state.toasts_idle(), "switching to drawing must not toast");

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightModeDrawing);

    assert!(state.light_mode_passthrough());
    assert!(
        state.toasts_idle(),
        "switching back to passthrough must not toast"
    );

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(!state.light_mode_active());
    assert!(state.toasts_idle(), "leaving light mode must not toast");

    assert!(state.set_light_mode_drawing(true));

    assert!(state.light_mode_drawing_active());
    assert!(
        state.toasts_idle(),
        "entering light mode to draw must not toast"
    );
}

#[test]
fn light_mode_unsupported_warning_ignores_disabled_toasts() {
    let test_text_measurer = TextMeasurer::default();
    let test_ui_engine = UiTextEngine::default();
    let test_text_resources = crate::input::state::InputTextResources {
        measurer: &test_text_measurer,
        ui_engine: &test_ui_engine,
    };
    let mut state = create_test_input_state();
    state.ui_visibility.show_light_mode_toasts = false;

    state.handle_action_with_resources(test_text_resources, Action::ToggleLightMode);

    assert!(!state.light_mode_active());
    assert!(
        state
            .test_active_toast_message()
            .is_some_and(|message| message.starts_with("Light Mode passthrough is not supported")),
        "the unsupported warning reports a failure, so it is always shown"
    );
}
