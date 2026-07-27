use wayscriber::config::{InputHudMode, InputHudPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputHudModeOption {
    Auto,
    Overlay,
    System,
}

impl InputHudModeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Auto, Self::Overlay, Self::System]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto (system when available)",
            Self::Overlay => "Overlay only",
            Self::System => "System-wide",
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_mode(&self) -> InputHudMode {
        match self {
            Self::Auto => InputHudMode::Auto,
            Self::Overlay => InputHudMode::Overlay,
            Self::System => InputHudMode::System,
        }
    }

    pub fn from_mode(mode: InputHudMode) -> Self {
        match mode {
            InputHudMode::Auto => Self::Auto,
            InputHudMode::Overlay => Self::Overlay,
            InputHudMode::System => Self::System,
        }
    }
}

impl std::fmt::Display for InputHudModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputHudPositionOption {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl InputHudPositionOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::TopLeft,
            Self::TopCenter,
            Self::TopRight,
            Self::CenterLeft,
            Self::Center,
            Self::CenterRight,
            Self::BottomLeft,
            Self::BottomCenter,
            Self::BottomRight,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::TopLeft => "Top Left",
            Self::TopCenter => "Top Center",
            Self::TopRight => "Top Right",
            Self::CenterLeft => "Center Left",
            Self::Center => "Center",
            Self::CenterRight => "Center Right",
            Self::BottomLeft => "Bottom Left",
            Self::BottomCenter => "Bottom Center",
            Self::BottomRight => "Bottom Right",
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_position(&self) -> InputHudPosition {
        match self {
            Self::TopLeft => InputHudPosition::TopLeft,
            Self::TopCenter => InputHudPosition::TopCenter,
            Self::TopRight => InputHudPosition::TopRight,
            Self::CenterLeft => InputHudPosition::CenterLeft,
            Self::Center => InputHudPosition::Center,
            Self::CenterRight => InputHudPosition::CenterRight,
            Self::BottomLeft => InputHudPosition::BottomLeft,
            Self::BottomCenter => InputHudPosition::BottomCenter,
            Self::BottomRight => InputHudPosition::BottomRight,
        }
    }

    pub fn from_position(position: InputHudPosition) -> Self {
        match position {
            InputHudPosition::TopLeft => Self::TopLeft,
            InputHudPosition::TopCenter => Self::TopCenter,
            InputHudPosition::TopRight => Self::TopRight,
            InputHudPosition::CenterLeft => Self::CenterLeft,
            InputHudPosition::Center => Self::Center,
            InputHudPosition::CenterRight => Self::CenterRight,
            InputHudPosition::BottomLeft => Self::BottomLeft,
            InputHudPosition::BottomCenter => Self::BottomCenter,
            InputHudPosition::BottomRight => Self::BottomRight,
        }
    }
}

impl std::fmt::Display for InputHudPositionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
