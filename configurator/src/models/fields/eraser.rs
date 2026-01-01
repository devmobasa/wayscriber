use wayscriber::draw::EraserKind;
use wayscriber::input::EraserMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraserModeOption {
    Brush,
    Stroke,
}

impl EraserModeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Brush, Self::Stroke]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Brush => "Brush",
            Self::Stroke => "Stroke",
        }
    }

    pub fn to_mode(self) -> EraserMode {
        match self {
            Self::Brush => EraserMode::Brush,
            Self::Stroke => EraserMode::Stroke,
        }
    }

    pub fn from_mode(mode: EraserMode) -> Self {
        match mode {
            EraserMode::Brush => EraserModeOption::Brush,
            EraserMode::Stroke => EraserModeOption::Stroke,
        }
    }
}

impl std::fmt::Display for EraserModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetEraserKindOption {
    Default,
    Circle,
    Rect,
}

impl PresetEraserKindOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Default, Self::Circle, Self::Rect]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Circle => "Circle",
            Self::Rect => "Rectangle",
        }
    }

    pub fn to_option(self) -> Option<EraserKind> {
        match self {
            Self::Default => None,
            Self::Circle => Some(EraserKind::Circle),
            Self::Rect => Some(EraserKind::Rect),
        }
    }

    pub fn from_option(value: Option<EraserKind>) -> Self {
        match value {
            None => Self::Default,
            Some(EraserKind::Circle) => Self::Circle,
            Some(EraserKind::Rect) => Self::Rect,
        }
    }
}

impl std::fmt::Display for PresetEraserKindOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetEraserModeOption {
    Default,
    Brush,
    Stroke,
}

impl PresetEraserModeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Default, Self::Brush, Self::Stroke]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Brush => "Brush",
            Self::Stroke => "Stroke",
        }
    }

    pub fn to_option(self) -> Option<EraserMode> {
        match self {
            Self::Default => None,
            Self::Brush => Some(EraserMode::Brush),
            Self::Stroke => Some(EraserMode::Stroke),
        }
    }

    pub fn from_option(value: Option<EraserMode>) -> Self {
        match value {
            None => Self::Default,
            Some(EraserMode::Brush) => Self::Brush,
            Some(EraserMode::Stroke) => Self::Stroke,
        }
    }
}

impl std::fmt::Display for PresetEraserModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
