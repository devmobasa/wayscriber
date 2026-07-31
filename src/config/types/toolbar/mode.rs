use serde::{Deserialize, Serialize};

/// Toolbar layout complexity presets.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarLayoutMode {
    Simple,
    #[default]
    #[serde(alias = "full")]
    Regular,
    Advanced,
}

impl ToolbarLayoutMode {
    /// The next preset in the cycle the strip's layout button advances
    /// through; wraps from Advanced back to Simple.
    pub fn next(self) -> Self {
        match self {
            Self::Simple => Self::Regular,
            Self::Regular => Self::Advanced,
            Self::Advanced => Self::Simple,
        }
    }

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
}

#[cfg(test)]
mod tests {
    use super::ToolbarLayoutMode;

    /// The layout cycle visits all three presets and wraps from Advanced
    /// back to Simple.
    #[test]
    fn next_cycles_through_every_preset_and_wraps() {
        assert_eq!(ToolbarLayoutMode::Simple.next(), ToolbarLayoutMode::Regular);
        assert_eq!(
            ToolbarLayoutMode::Regular.next(),
            ToolbarLayoutMode::Advanced
        );
        assert_eq!(
            ToolbarLayoutMode::Advanced.next(),
            ToolbarLayoutMode::Simple
        );
    }
}
