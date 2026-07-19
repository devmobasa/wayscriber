use wayscriber::config::{ToolbarLayoutMode, ToolbarRebindModifier, ToolbarSideLayout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarRebindModifierOption {
    Disabled,
    CtrlShift,
    CtrlAlt,
    ShiftAlt,
    CtrlShiftAlt,
}

impl ToolbarRebindModifierOption {
    pub const ALL: [Self; 5] = [
        Self::CtrlShift,
        Self::CtrlAlt,
        Self::ShiftAlt,
        Self::CtrlShiftAlt,
        Self::Disabled,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::CtrlShift => "Ctrl+Shift",
            Self::CtrlAlt => "Ctrl+Alt",
            Self::ShiftAlt => "Shift+Alt",
            Self::CtrlShiftAlt => "Ctrl+Shift+Alt",
        }
    }

    pub fn from_config(value: ToolbarRebindModifier) -> Self {
        match value {
            ToolbarRebindModifier::Disabled => Self::Disabled,
            ToolbarRebindModifier::CtrlShift => Self::CtrlShift,
            ToolbarRebindModifier::CtrlAlt => Self::CtrlAlt,
            ToolbarRebindModifier::ShiftAlt => Self::ShiftAlt,
            ToolbarRebindModifier::CtrlShiftAlt => Self::CtrlShiftAlt,
        }
    }

    pub fn to_config(self) -> ToolbarRebindModifier {
        match self {
            Self::Disabled => ToolbarRebindModifier::Disabled,
            Self::CtrlShift => ToolbarRebindModifier::CtrlShift,
            Self::CtrlAlt => ToolbarRebindModifier::CtrlAlt,
            Self::ShiftAlt => ToolbarRebindModifier::ShiftAlt,
            Self::CtrlShiftAlt => ToolbarRebindModifier::CtrlShiftAlt,
        }
    }
}

impl std::fmt::Display for ToolbarRebindModifierOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

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
pub enum ToolbarSideLayoutOption {
    Pill,
    Panel,
}

impl ToolbarSideLayoutOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Panel, Self::Pill]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Pill => "Pill (preview)",
            Self::Panel => "Panel (default)",
        }
    }

    pub fn to_config(self) -> ToolbarSideLayout {
        match self {
            Self::Pill => ToolbarSideLayout::Pill,
            Self::Panel => ToolbarSideLayout::Panel,
        }
    }

    pub fn from_config(value: ToolbarSideLayout) -> Self {
        match value {
            ToolbarSideLayout::Pill => Self::Pill,
            ToolbarSideLayout::Panel => Self::Panel,
        }
    }
}

impl std::fmt::Display for ToolbarSideLayoutOption {
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
    ShowZoomActions,
    ShowPagesSection,
    ShowBoardsSection,
    ShowStepSection,
    ShowTextControls,
}

impl ToolbarOverrideField {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ShowPresets => "Presets",
            Self::ShowActionsSection => "Actions",
            Self::ShowActionsAdvanced => "Advanced actions",
            Self::ShowZoomActions => "Zoom actions",
            Self::ShowPagesSection => "Pages",
            Self::ShowBoardsSection => "Boards",
            Self::ShowStepSection => "Multi-step undo/redo",
            Self::ShowTextControls => "Text controls",
        }
    }
}
