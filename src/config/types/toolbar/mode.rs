use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Toolbar layout complexity presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarLayoutMode {
    Simple,
    #[default]
    #[serde(alias = "full")]
    Regular,
    Advanced,
}

impl ToolbarLayoutMode {
    pub fn section_defaults(self) -> ToolbarSectionDefaults {
        match self {
            Self::Simple => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: false,
                show_zoom_actions: true,
                show_pages_section: true,
                show_boards_section: true,
                show_presets: false,
                show_step_section: false,
                show_text_controls: true,
                show_settings_section: false,
            },
            Self::Regular => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: false,
                show_zoom_actions: true,
                show_pages_section: true,
                show_boards_section: true,
                show_presets: true,
                show_step_section: false,
                show_text_controls: true,
                show_settings_section: true,
            },
            Self::Advanced => ToolbarSectionDefaults {
                show_actions_section: true,
                show_actions_advanced: true,
                show_zoom_actions: true,
                show_pages_section: true,
                show_boards_section: true,
                show_presets: true,
                show_step_section: true,
                show_text_controls: true,
                show_settings_section: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToolbarSectionDefaults {
    pub show_actions_section: bool,
    pub show_actions_advanced: bool,
    pub show_zoom_actions: bool,
    pub show_pages_section: bool,
    pub show_boards_section: bool,
    pub show_presets: bool,
    pub show_step_section: bool,
    pub show_text_controls: bool,
    pub show_settings_section: bool,
}
