use super::create_test_input_state;
use crate::config::Action;
use crate::input::Tool;

fn create_light_mode_test_state() -> crate::input::InputState {
    let mut state = create_test_input_state();
    state.compositor_capabilities.layer_shell = true;
    state
}

#[test]
fn light_mode_enters_passthrough_and_hides_heavy_ui() {
    let mut state = create_light_mode_test_state();
    state.show_status_bar = true;
    state.toolbar_visible = true;
    state.toolbar_top_visible = true;
    state.toolbar_side_visible = true;
    state.show_tool_preview = true;
    state.set_tool_override(Some(Tool::Arrow));

    state.handle_action(Action::ToggleLightMode);

    assert!(state.light_mode);
    assert!(!state.light_mode_drawing);
    assert!(state.light_mode_passthrough());
    assert!(!state.show_status_bar);
    assert!(!state.toolbar_visible);
    assert!(!state.toolbar_top_visible);
    assert!(!state.toolbar_side_visible);
    assert!(!state.show_tool_preview);
    assert_eq!(state.tool_override(), Some(Tool::Pen));
}

#[test]
fn light_mode_drawing_toggle_disables_passthrough_without_exiting() {
    let mut state = create_light_mode_test_state();

    state.handle_action(Action::ToggleLightMode);
    state.handle_action(Action::ToggleLightModeDrawing);

    assert!(state.light_mode);
    assert!(state.light_mode_drawing);
    assert!(!state.light_mode_passthrough());

    state.handle_action(Action::ToggleLightModeDrawing);

    assert!(state.light_mode);
    assert!(!state.light_mode_drawing);
    assert!(state.light_mode_passthrough());
}

#[test]
fn light_draw_off_does_not_enter_light_mode() {
    let mut state = create_light_mode_test_state();

    assert!(!state.set_light_mode_drawing(false));

    assert!(!state.light_mode);
    assert!(!state.light_mode_drawing);
}

#[test]
fn light_mode_restores_previous_ui_and_tool_on_exit() {
    let mut state = create_light_mode_test_state();
    state.show_status_bar = true;
    state.toolbar_visible = true;
    state.toolbar_top_visible = false;
    state.toolbar_side_visible = true;
    state.show_tool_preview = true;
    state.set_tool_override(Some(Tool::Marker));

    state.handle_action(Action::ToggleLightMode);
    state.handle_action(Action::ToggleLightMode);

    assert!(!state.light_mode);
    assert!(!state.light_mode_drawing);
    assert!(!state.light_mode_passthrough());
    assert!(state.show_status_bar);
    assert!(state.toolbar_visible);
    assert!(!state.toolbar_top_visible);
    assert!(state.toolbar_side_visible);
    assert!(state.show_tool_preview);
    assert_eq!(state.tool_override(), Some(Tool::Marker));
}

#[test]
fn light_mode_and_presenter_mode_are_mutually_exclusive() {
    let mut state = create_light_mode_test_state();

    state.handle_action(Action::TogglePresenterMode);
    assert!(state.presenter_mode);

    state.handle_action(Action::ToggleLightMode);

    assert!(state.light_mode);
    assert!(!state.presenter_mode);
}

#[test]
fn light_mode_does_not_enter_without_layer_shell() {
    let mut state = create_test_input_state();

    state.handle_action(Action::ToggleLightMode);

    assert!(!state.light_mode);
    assert!(!state.light_mode_passthrough());
}
