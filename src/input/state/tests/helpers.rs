use super::*;

pub(super) fn create_test_input_state() -> InputState {
    create_test_input_state_with_keybindings(crate::config::KeybindingsConfig::default())
}

pub(super) fn create_test_input_state_with_click_highlight(
    click_highlight_settings: ClickHighlightSettings,
) -> InputState {
    create_test_input_state_with_keybindings_and_click_highlight(
        crate::config::KeybindingsConfig::default(),
        click_highlight_settings,
    )
}

pub(super) fn create_test_input_state_with_keybindings(
    keybindings: crate::config::KeybindingsConfig,
) -> InputState {
    create_test_input_state_with_keybindings_and_click_highlight(
        keybindings,
        ClickHighlightSettings::disabled(),
    )
}

fn create_test_input_state_with_keybindings_and_click_highlight(
    keybindings: crate::config::KeybindingsConfig,
    click_highlight_settings: ClickHighlightSettings,
) -> InputState {
    let action_map = keybindings.build_action_map().unwrap();
    let action_bindings = keybindings.build_action_bindings().unwrap();

    let mut state = InputState::from_seed(crate::input::InputStateSeed {
        color: Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thickness: 3.0,
        eraser_size: 12.0,
        eraser_mode: EraserMode::Brush,
        marker_opacity: 0.32,
        fill_enabled: false,
        font_size: 32.0,
        font_descriptor: FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        },
        text_background_enabled: false,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        arrow_head_at_end: false,
        show_status_bar: true,
        boards_config: BoardsConfig::default(),
        action_map: action_map,
        max_shapes_per_frame: usize::MAX,
        click_highlight_settings: click_highlight_settings,
        undo_all_delay_ms: 0,
        redo_all_delay_ms: 0,
        custom_section_enabled: false,
        custom_undo_delay_ms: 0,
        custom_redo_delay_ms: 0,
        custom_undo_steps: 5,
        custom_redo_steps: 5,
        presenter_mode_config: crate::config::PresenterModeConfig::default(),
    });
    state.set_action_bindings(action_bindings);
    state
}
