use super::keybindings::KeybindingsConfig;
#[cfg(tablet)]
use super::types::TabletInputConfig;
use super::types::{
    ArrowConfig, BoardConfig, CaptureConfig, DrawingConfig, HistoryConfig, PerformanceConfig,
    PresetSlotsConfig, SessionConfig, UiConfig,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Main configuration structure containing all user settings.
///
/// This is the root configuration type that gets deserialized from the TOML file.
/// All fields have sensible defaults and will use those if not specified in the config file.
///
/// # Example TOML
/// ```toml
/// [drawing]
/// default_color = "red"
/// default_thickness = 3.0
/// default_font_size = 32.0
///
/// [arrow]
/// length = 20.0
/// angle_degrees = 30.0
/// head_at_end = false
///
/// [performance]
/// buffer_count = 3
/// enable_vsync = true
/// ui_animation_fps = 30
///
/// [ui]
/// show_status_bar = true
/// status_bar_position = "bottom-left"
///
/// [keybindings]
/// exit = ["Escape", "Ctrl+Q"]
/// undo = ["Ctrl+Z"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct Config {
    /// Drawing tool defaults (color, thickness, font size)
    #[serde(default)]
    pub drawing: DrawingConfig,

    /// Preset slots for quick tool switching
    #[serde(default)]
    pub presets: PresetSlotsConfig,

    /// History playback settings
    #[serde(default)]
    pub history: HistoryConfig,

    /// Arrow appearance settings
    #[serde(default)]
    pub arrow: ArrowConfig,

    /// Performance tuning options
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// UI display preferences
    #[serde(default)]
    pub ui: UiConfig,

    /// Board mode settings (whiteboard/blackboard)
    #[serde(default)]
    pub board: BoardConfig,

    /// Keybinding customization
    #[serde(default)]
    pub keybindings: KeybindingsConfig,

    /// Screenshot capture settings
    #[serde(default)]
    pub capture: CaptureConfig,

    /// Tablet/stylus input settings (feature-gated)
    #[cfg(tablet)]
    #[serde(default)]
    pub tablet: TabletInputConfig,

    /// Session persistence settings
    #[serde(default)]
    pub session: SessionConfig,
}
