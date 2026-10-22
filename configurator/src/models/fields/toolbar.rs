use std::fmt;
use std::fmt::{Display, Formatter};

use wayscriber::i18n::tr;

use wayscriber::config::{
    ToolbarLayoutMode, ToolbarRebindModifier, ToolbarStrokeControls, ZoomChipDisplay,
};

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
            Self::Disabled => tr("Disabled"),
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

impl Display for ToolbarRebindModifierOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            ToolbarLayoutModeOption::Simple => tr("Simple"),
            ToolbarLayoutModeOption::Regular => tr("Regular"),
            ToolbarLayoutModeOption::Advanced => tr("Advanced"),
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

impl Display for ToolbarLayoutModeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomChipDisplayOption {
    Always,
    WhileZoomed,
}

impl ZoomChipDisplayOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Always, Self::WhileZoomed]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Always => tr("Always (default)"),
            Self::WhileZoomed => tr("Only while zoomed"),
        }
    }

    pub fn to_config(self) -> ZoomChipDisplay {
        match self {
            Self::Always => ZoomChipDisplay::Always,
            Self::WhileZoomed => ZoomChipDisplay::WhileZoomed,
        }
    }

    pub fn from_config(value: ZoomChipDisplay) -> Self {
        match value {
            ZoomChipDisplay::Always => Self::Always,
            ZoomChipDisplay::WhileZoomed => Self::WhileZoomed,
        }
    }
}

impl Display for ZoomChipDisplayOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// How the toolbar's style pill shows pen smoothing and Shape Pen
/// sensitivity (`[ui.toolbar] stroke_controls`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarStrokeControlsOption {
    Panel,
    Meter,
    Stepper,
}

impl ToolbarStrokeControlsOption {
    pub fn list() -> Vec<Self> {
        ToolbarStrokeControls::ALL
            .into_iter()
            .map(Self::from_config)
            .collect()
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Panel => tr("Panel (default)"),
            Self::Meter => tr("Meters"),
            Self::Stepper => tr("Steppers"),
        }
    }

    pub fn to_config(self) -> ToolbarStrokeControls {
        match self {
            Self::Panel => ToolbarStrokeControls::Panel,
            Self::Meter => ToolbarStrokeControls::Meter,
            Self::Stepper => ToolbarStrokeControls::Stepper,
        }
    }

    pub fn from_config(value: ToolbarStrokeControls) -> Self {
        match value {
            ToolbarStrokeControls::Panel => Self::Panel,
            ToolbarStrokeControls::Meter => Self::Meter,
            ToolbarStrokeControls::Stepper => Self::Stepper,
        }
    }
}

impl Display for ToolbarStrokeControlsOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            Self::Default => tr("Default"),
            Self::On => tr("On"),
            Self::Off => tr("Off"),
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

impl Display for OverrideOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
            Self::ShowPresets => tr("Presets"),
            Self::ShowActionsSection => tr("Actions"),
            Self::ShowActionsAdvanced => tr("Advanced actions"),
            Self::ShowZoomActions => tr("Zoom actions"),
            Self::ShowPagesSection => tr("Pages"),
            Self::ShowBoardsSection => tr("Boards"),
            Self::ShowStepSection => tr("Multi-step undo/redo"),
            Self::ShowTextControls => tr("Text controls"),
        }
    }
}
