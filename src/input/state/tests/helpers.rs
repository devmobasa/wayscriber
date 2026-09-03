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
    crate::input::state::test_support::TestInputStateBuilder::with_keybindings(keybindings)
        .thickness(3.0)
        .eraser_size(12.0)
        .font_descriptor(FontDescriptor {
            family: "Sans".to_string(),
            weight: "bold".to_string(),
            style: "normal".to_string(),
        })
        .click_highlight_settings(click_highlight_settings)
        .custom_section_enabled(false)
        .build()
}
