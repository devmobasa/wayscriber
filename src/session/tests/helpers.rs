use crate::config::{Action, BoardsConfig, PresenterModeConfig, Shortcut};
use crate::draw::Color as DrawColor;
use crate::draw::FontDescriptor;
use crate::input::{ClickHighlightSettings, EraserMode, InputState};
use std::collections::HashMap;

pub(super) fn dummy_input_state() -> InputState {
    let mut action_map = HashMap::new();
    action_map.insert(Shortcut::parse("Escape").unwrap(), Action::Exit);
    InputState::from_seed(crate::input::InputStateSeed {
        color: DrawColor {
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
        font_descriptor: FontDescriptor::default(),
        text_background_enabled: false,
        arrow_length: 20.0,
        arrow_angle: 30.0,
        arrow_head_at_end: false,
        show_status_bar: true,
        boards_config: BoardsConfig::default(),
        action_map: action_map,
        max_shapes_per_frame: usize::MAX,
        click_highlight_settings: ClickHighlightSettings::disabled(),
        undo_all_delay_ms: 0,
        redo_all_delay_ms: 0,
        custom_section_enabled: true,
        custom_undo_delay_ms: 0,
        custom_redo_delay_ms: 0,
        custom_undo_steps: 5,
        custom_redo_steps: 5,
        presenter_mode_config: PresenterModeConfig::default(),
    })
}
