use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ToolbarLayoutMode;

/// Optional per-mode overrides for toolbar sections.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
pub struct ToolbarModeOverride {
    /// Show the Actions section (undo/redo/clear)
    #[serde(default)]
    pub show_actions_section: Option<bool>,

    /// Show advanced action buttons (undo all, delay, freeze, etc.)
    #[serde(default)]
    pub show_actions_advanced: Option<bool>,

    /// Show the Pages section in the side toolbar
    #[serde(default)]
    pub show_pages_section: Option<bool>,

    /// Show the presets section in the side toolbar
    #[serde(default)]
    pub show_presets: Option<bool>,

    /// Show the Step Undo/Redo section
    #[serde(default)]
    pub show_step_section: Option<bool>,

    /// Keep text controls visible even when text is not active
    #[serde(default)]
    pub show_text_controls: Option<bool>,

    /// Show the Settings section (config shortcuts, layout controls)
    #[serde(default)]
    pub show_settings_section: Option<bool>,
}

/// Mode-specific overrides for toolbar layout presets.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct ToolbarModeOverrides {
    #[serde(default)]
    pub simple: ToolbarModeOverride,
    #[serde(default, alias = "full")]
    pub regular: ToolbarModeOverride,
    #[serde(default)]
    pub advanced: ToolbarModeOverride,
}

impl ToolbarModeOverrides {
    pub fn for_mode(&self, mode: ToolbarLayoutMode) -> &ToolbarModeOverride {
        match mode {
            ToolbarLayoutMode::Simple => &self.simple,
            ToolbarLayoutMode::Regular => &self.regular,
            ToolbarLayoutMode::Advanced => &self.advanced,
        }
    }
}
