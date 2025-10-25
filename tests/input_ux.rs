use wayscriber::config::{BoardConfig, KeybindingsConfig};
use wayscriber::draw::{Color, FontDescriptor};
use wayscriber::input::{DrawingState, InputState, Key, MouseButton, SystemCommand};

fn make_input_state() -> InputState {
    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().expect("action map");
    InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        4.0,
        32.0,
        FontDescriptor::default(),
        false,
        20.0,
        30.0,
        BoardConfig::default(),
        action_map,
    )
}

#[test]
fn font_size_adjustment_sets_redraw_and_clamps() {
    let mut state = make_input_state();
    state.needs_redraw = false;

    state.adjust_font_size(100.0);
    assert_eq!(state.current_font_size, 72.0, "font size clamps to max");
    assert!(state.needs_redraw, "status bar should redraw after resize");

    state.needs_redraw = false;
    state.adjust_font_size(-100.0);
    assert_eq!(state.current_font_size, 8.0, "font size clamps to min");
    assert!(state.needs_redraw, "text cursor should redraw after resize");
}

#[test]
fn escape_in_drawing_mode_cancels_stroke_without_exiting() {
    let mut state = make_input_state();
    state.on_mouse_press(MouseButton::Left, 0, 0);
    assert!(matches!(state.state, DrawingState::Drawing { .. }));

    state.on_key_press(Key::Escape);
    assert!(matches!(state.state, DrawingState::Idle));
    assert!(
        state.needs_redraw,
        "cancelling a stroke should trigger redraw"
    );
    assert!(
        !state.should_exit,
        "Escape during drawing cancels stroke rather than closing overlay"
    );
}

#[test]
fn pressing_f11_requests_overlay_exit_before_launching_configurator() {
    let mut state = make_input_state();
    assert!(
        !state.should_exit,
        "sanity check: overlay should not be exiting before F11"
    );
    assert!(state.take_pending_system_command().is_none());

    state.on_key_press(Key::F11);

    assert!(
        state.should_exit,
        "Pressing F11 should request overlay exit before spawning the configurator"
    );

    assert_eq!(
        state.take_pending_system_command(),
        Some(SystemCommand::LaunchConfigurator)
    );
    assert!(
        state.take_pending_system_command().is_none(),
        "command should clear once consumed"
    );
}

#[test]
fn escape_during_capture_keeps_overlay_visible_until_capture_completes() {
    let mut state = make_input_state();
    state.begin_capture_guard();

    state.on_key_press(Key::Escape);

    assert!(
        !state.should_exit,
        "Escape should not close overlay while capture guard is active"
    );

    state.end_capture_guard();
    state.on_key_press(Key::Escape);
    assert!(
        state.should_exit,
        "Exit should function once capture completes"
    );
}

#[test]
fn pressing_f10_toggles_help_overlay() {
    let mut state = make_input_state();
    assert!(!state.show_help);
    state.on_key_press(Key::F10);
    assert!(state.show_help);
    state.on_key_press(Key::F10);
    assert!(!state.show_help);
}

#[test]
#[ignore = "Pending multi-output surface coverage"]
fn overlay_creation_handles_multiple_outputs() {
    todo!(
        "Add a compositor harness to assert per-output layer surfaces once multi-monitor support is implemented"
    );
}
