use wayscriber::input::state::{PressureThicknessEditMode, PressureThicknessEntryMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureThicknessEditModeOption {
    Disabled,
    Add,
    Scale,
}

impl PressureThicknessEditModeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Disabled, Self::Add, Self::Scale]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::Add => "Add thickness",
            Self::Scale => "Scale thickness",
        }
    }

    pub fn to_mode(self) -> PressureThicknessEditMode {
        match self {
            Self::Disabled => PressureThicknessEditMode::Disabled,
            Self::Add => PressureThicknessEditMode::Add,
            Self::Scale => PressureThicknessEditMode::Scale,
        }
    }

    pub fn from_mode(mode: PressureThicknessEditMode) -> Self {
        match mode {
            PressureThicknessEditMode::Disabled => Self::Disabled,
            PressureThicknessEditMode::Add => Self::Add,
            PressureThicknessEditMode::Scale => Self::Scale,
        }
    }
}

impl std::fmt::Display for PressureThicknessEditModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureThicknessEntryModeOption {
    Never,
    PressureOnly,
    AnyPressure,
}

impl PressureThicknessEntryModeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Never, Self::PressureOnly, Self::AnyPressure]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Never => "Never",
            Self::PressureOnly => "Pressure-only",
            Self::AnyPressure => "Any pressure",
        }
    }

    pub fn to_mode(self) -> PressureThicknessEntryMode {
        match self {
            Self::Never => PressureThicknessEntryMode::Never,
            Self::PressureOnly => PressureThicknessEntryMode::PressureOnly,
            Self::AnyPressure => PressureThicknessEntryMode::AnyPressure,
        }
    }

    pub fn from_mode(mode: PressureThicknessEntryMode) -> Self {
        match mode {
            PressureThicknessEntryMode::Never => Self::Never,
            PressureThicknessEntryMode::PressureOnly => Self::PressureOnly,
            PressureThicknessEntryMode::AnyPressure => Self::AnyPressure,
        }
    }
}

impl std::fmt::Display for PressureThicknessEntryModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
