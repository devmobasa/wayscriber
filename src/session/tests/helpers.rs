use crate::config::{Action, BoardsConfig, KeyBinding, PresenterModeConfig};
use crate::draw::Color as DrawColor;
use crate::draw::FontDescriptor;
use crate::input::{ClickHighlightSettings, EraserMode, InputState};
use std::collections::HashMap;

pub(super) fn dummy_input_state() -> InputState {
    let mut action_map = HashMap::new();
    action_map.insert(KeyBinding::parse("Escape").unwrap(), Action::Exit);
    InputState::with_defaults(
        DrawColor {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        3.0,
        12.0,
        EraserMode::Brush,
        0.32,
        false,
        32.0,
        FontDescriptor::default(),
        false,
        20.0,
        30.0,
        false,
        true,
        BoardsConfig::default(),
        action_map,
        usize::MAX,
        ClickHighlightSettings::disabled(),
        0,
        0,
        true,
        0,
        0,
        5,
        5,
        PresenterModeConfig::default(),
    )
}
