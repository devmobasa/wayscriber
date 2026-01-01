use wayscriber::config::ToolbarLayoutMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarLayoutModeOption {
    Simple,
    Regular,
    Advanced,
}

impl ToolbarLayoutModeOption {
    pub fn list() -> Vec<Self> {
        vec![
            ToolbarLayoutModeOption::Simple,
            ToolbarLayoutModeOption::Regular,
            ToolbarLayoutModeOption::Advanced,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ToolbarLayoutModeOption::Simple => "Simple",
            ToolbarLayoutModeOption::Regular => "Regular",
            ToolbarLayoutModeOption::Advanced => "Advanced",
        }
    }

    pub fn to_mode(self) -> ToolbarLayoutMode {
        match self {
            ToolbarLayoutModeOption::Simple => ToolbarLayoutMode::Simple,
            ToolbarLayoutModeOption::Regular => ToolbarLayoutMode::Regular,
            ToolbarLayoutModeOption::Advanced => ToolbarLayoutMode::Advanced,
        }
    }

    pub fn from_mode(mode: ToolbarLayoutMode) -> Self {
        match mode {
            ToolbarLayoutMode::Simple => ToolbarLayoutModeOption::Simple,
            ToolbarLayoutMode::Regular => ToolbarLayoutModeOption::Regular,
            ToolbarLayoutMode::Advanced => ToolbarLayoutModeOption::Advanced,
        }
    }
}

impl std::fmt::Display for ToolbarLayoutModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideOption {
    Default,
    On,
    Off,
}

impl OverrideOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Default, Self::On, Self::Off]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::On => "On",
            Self::Off => "Off",
        }
    }

    pub fn from_option(value: Option<bool>) -> Self {
        match value {
            Some(true) => Self::On,
            Some(false) => Self::Off,
            None => Self::Default,
        }
    }

    pub fn to_option(self) -> Option<bool> {
        match self {
            Self::Default => None,
            Self::On => Some(true),
            Self::Off => Some(false),
        }
    }
}

impl std::fmt::Display for OverrideOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ToolbarOverrideField {
    ShowPresets,
    ShowActionsSection,
    ShowActionsAdvanced,
    ShowPagesSection,
    ShowStepSection,
    ShowTextControls,
    ShowSettingsSection,
}

impl ToolbarOverrideField {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ShowPresets => "Presets",
            Self::ShowActionsSection => "Actions (basic)",
            Self::ShowActionsAdvanced => "Actions (advanced)",
            Self::ShowPagesSection => "Pages",
            Self::ShowStepSection => "Step controls",
            Self::ShowTextControls => "Text controls",
            Self::ShowSettingsSection => "Settings section",
        }
    }
}
