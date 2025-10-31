//! Shared helpers for backend implementations (config loading, input state setup, etc.).

use anyhow::{Result, anyhow};
use log::{info, warn};

use crate::{
    config::{Config, ConfigSource},
    input::{BoardMode, ClickHighlightSettings, InputState},
};

/// Load configuration, falling back to defaults on error.
pub fn load_config() -> (Config, ConfigSource) {
    match Config::load() {
        Ok(loaded) => (loaded.config, loaded.source),
        Err(e) => {
            warn!("Failed to load config: {}. Using defaults.", e);
            (Config::default(), ConfigSource::Default)
        }
    }
}

/// Builds an `InputState` using the provided configuration and optional initial board mode.
pub fn build_input_state(config: &Config, initial_mode: Option<String>) -> Result<InputState> {
    let font_descriptor = crate::draw::FontDescriptor::new(
        config.drawing.font_family.clone(),
        config.drawing.font_weight.clone(),
        config.drawing.font_style.clone(),
    );

    let action_map = config
        .keybindings
        .build_action_map()
        .map_err(|err| anyhow!("Failed to build keybinding action map: {}", err))?;

    let mut input_state = InputState::with_defaults(
        config.drawing.default_color.to_color(),
        config.drawing.default_thickness,
        config.drawing.default_font_size,
        font_descriptor,
        config.drawing.text_background_enabled,
        config.arrow.length,
        config.arrow.angle_degrees,
        config.ui.show_status_bar,
        config.board.clone(),
        action_map,
        config.session.max_shapes_per_frame,
        ClickHighlightSettings::from(&config.ui.click_highlight),
    );

    // Apply initial mode from CLI (if provided) or config default (only if board modes enabled)
    if config.board.enabled {
        let initial_mode_str = initial_mode
            .clone()
            .unwrap_or_else(|| config.board.default_mode.clone());

        if let Ok(mode) = initial_mode_str.parse::<BoardMode>() {
            if mode != BoardMode::Transparent {
                info!("Starting in {} mode", initial_mode_str);
                input_state.canvas_set.switch_mode(mode);
                // Apply auto-color adjustment if enabled
                if config.board.auto_adjust_pen {
                    if let Some(default_color) = mode.default_pen_color(&config.board) {
                        input_state.current_color = default_color;
                    }
                }
            }
        } else if !initial_mode_str.is_empty() {
            warn!("Invalid board mode '{}'", initial_mode_str);
        }
    } else if initial_mode.is_some() {
        warn!("Board modes disabled in config, ignoring requested initial mode");
    }

    Ok(input_state)
}
