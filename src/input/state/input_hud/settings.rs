use std::time::Duration;

use crate::config::{InputHudConfig, InputHudMode, InputHudPosition};

/// Runtime settings for the on-screen input HUD.
#[derive(Clone)]
pub struct InputHudSettings {
    pub enabled: bool,
    pub mode: InputHudMode,
    pub position: InputHudPosition,
    pub show_mouse: bool,
    pub show_bare_modifiers: bool,
    /// Hold time after the last press before a chip starts fading.
    pub display: Duration,
    /// Fade-out duration once the hold time has elapsed.
    pub fade: Duration,
    pub max_entries: usize,
    pub combine_repeats: bool,
    pub font_size: f64,
}

impl Default for InputHudSettings {
    fn default() -> Self {
        Self::from(&InputHudConfig::default())
    }
}

impl From<&InputHudConfig> for InputHudSettings {
    fn from(cfg: &InputHudConfig) -> Self {
        Self {
            enabled: cfg.enabled,
            mode: cfg.mode,
            position: cfg.position,
            show_mouse: cfg.show_mouse,
            show_bare_modifiers: cfg.show_bare_modifiers,
            display: Duration::from_millis(cfg.display_ms),
            fade: Duration::from_millis(cfg.fade_ms),
            // A zero cap would make every note a no-op; the config validator
            // already clamps user input, so this only guards direct callers.
            max_entries: cfg.max_entries.max(1),
            combine_repeats: cfg.combine_repeats,
            font_size: cfg.font_size,
        }
    }
}
