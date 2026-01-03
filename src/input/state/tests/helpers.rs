use super::*;

pub(super) fn create_test_input_state() -> InputState {
    use crate::config::KeybindingsConfig;

    let keybindings = KeybindingsConfig::default();
    let action_map = keybindings.build_action_map().unwrap();

    InputState::with_defaults(
        Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }, // Red
        3.0,  // thickness
        12.0, // eraser size
        EraserMode::Brush,
        0.32,  // marker_opacity
        false, // fill_enabled
        32.0,  // font_size
        FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        },
        false,                  // text_background_enabled
        20.0,                   // arrow_length
        30.0,                   // arrow_angle
        false,                  // arrow_head_at_end
        true,                   // show_status_bar
        BoardConfig::default(), // board_config
        action_map,             // action_map
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        false, // custom_section_enabled
        0,     // custom_undo_delay_ms
        0,     // custom_redo_delay_ms
        5,     // custom_undo_steps
        5,     // custom_redo_steps
        crate::config::PresenterModeConfig::default(),
    )
}
