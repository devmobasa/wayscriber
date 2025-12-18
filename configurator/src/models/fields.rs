use wayscriber::config::{SessionCompression, SessionStorageMode, StatusPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontStyleOption {
    Normal,
    Italic,
    Oblique,
    Custom,
}

impl FontStyleOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Normal, Self::Italic, Self::Oblique, Self::Custom]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Italic => "Italic",
            Self::Oblique => "Oblique",
            Self::Custom => "Custom",
        }
    }

    pub fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Italic => "italic",
            Self::Oblique => "oblique",
            Self::Custom => "",
        }
    }

    pub fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, "normal".to_string()),
            "italic" => (Self::Italic, "italic".to_string()),
            "oblique" => (Self::Oblique, "oblique".to_string()),
            _ => (Self::Custom, value.to_string()),
        }
    }
}

impl std::fmt::Display for FontStyleOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeightOption {
    Normal,
    Bold,
    Light,
    Ultralight,
    Heavy,
    Ultrabold,
    Custom,
}

impl FontWeightOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Normal,
            Self::Bold,
            Self::Light,
            Self::Ultralight,
            Self::Heavy,
            Self::Ultrabold,
            Self::Custom,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Bold => "Bold",
            Self::Light => "Light",
            Self::Ultralight => "Ultralight",
            Self::Heavy => "Heavy",
            Self::Ultrabold => "Ultrabold",
            Self::Custom => "Custom",
        }
    }

    pub fn canonical_value(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Bold => "bold",
            Self::Light => "light",
            Self::Ultralight => "ultralight",
            Self::Heavy => "heavy",
            Self::Ultrabold => "ultrabold",
            Self::Custom => "",
        }
    }

    pub fn from_value(value: &str) -> (Self, String) {
        let lower = value.trim().to_lowercase();
        match lower.as_str() {
            "normal" => (Self::Normal, "normal".to_string()),
            "bold" => (Self::Bold, "bold".to_string()),
            "light" => (Self::Light, "light".to_string()),
            "ultralight" => (Self::Ultralight, "ultralight".to_string()),
            "heavy" => (Self::Heavy, "heavy".to_string()),
            "ultrabold" => (Self::Ultrabold, "ultrabold".to_string()),
            _ => (Self::Custom, value.to_string()),
        }
    }
}

impl std::fmt::Display for FontWeightOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusPositionOption {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl StatusPositionOption {
    pub fn list() -> Vec<Self> {
        vec![
            StatusPositionOption::TopLeft,
            StatusPositionOption::TopRight,
            StatusPositionOption::BottomLeft,
            StatusPositionOption::BottomRight,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            StatusPositionOption::TopLeft => "Top Left",
            StatusPositionOption::TopRight => "Top Right",
            StatusPositionOption::BottomLeft => "Bottom Left",
            StatusPositionOption::BottomRight => "Bottom Right",
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_status_position(&self) -> StatusPosition {
        match self {
            StatusPositionOption::TopLeft => StatusPosition::TopLeft,
            StatusPositionOption::TopRight => StatusPosition::TopRight,
            StatusPositionOption::BottomLeft => StatusPosition::BottomLeft,
            StatusPositionOption::BottomRight => StatusPosition::BottomRight,
        }
    }

    pub fn from_status_position(position: StatusPosition) -> Self {
        match position {
            StatusPosition::TopLeft => StatusPositionOption::TopLeft,
            StatusPosition::TopRight => StatusPositionOption::TopRight,
            StatusPosition::BottomLeft => StatusPositionOption::BottomLeft,
            StatusPosition::BottomRight => StatusPositionOption::BottomRight,
        }
    }
}

impl std::fmt::Display for StatusPositionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardModeOption {
    Transparent,
    Whiteboard,
    Blackboard,
}

impl BoardModeOption {
    pub fn list() -> Vec<Self> {
        vec![
            BoardModeOption::Transparent,
            BoardModeOption::Whiteboard,
            BoardModeOption::Blackboard,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            BoardModeOption::Transparent => "Transparent",
            BoardModeOption::Whiteboard => "Whiteboard",
            BoardModeOption::Blackboard => "Blackboard",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            BoardModeOption::Transparent => "transparent",
            BoardModeOption::Whiteboard => "whiteboard",
            BoardModeOption::Blackboard => "blackboard",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "transparent" => Some(BoardModeOption::Transparent),
            "whiteboard" => Some(BoardModeOption::Whiteboard),
            "blackboard" => Some(BoardModeOption::Blackboard),
            _ => None,
        }
    }
}

impl std::fmt::Display for BoardModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleField {
    DrawingTextBackground,
    PerformanceVsync,
    UiShowStatusBar,
    UiShowFrozenBadge,
    UiClickHighlightEnabled,
    UiClickHighlightUsePenColor,
    BoardEnabled,
    BoardAutoAdjust,
    CaptureEnabled,
    CaptureCopyToClipboard,
    SessionPersistTransparent,
    SessionPersistWhiteboard,
    SessionPersistBlackboard,
    SessionRestoreToolState,
    SessionPerOutput,
    ArrowHeadAtEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    DrawingColorName,
    DrawingThickness,
    DrawingFontSize,
    DrawingFontFamily,
    DrawingFontWeight,
    DrawingFontStyle,
    ArrowLength,
    ArrowAngle,
    StatusFontSize,
    StatusPadding,
    StatusDotRadius,
    HighlightRadius,
    HighlightOutlineThickness,
    HighlightDurationMs,
    HelpFontSize,
    HelpLineHeight,
    HelpPadding,
    HelpBorderWidth,
    CaptureSaveDirectory,
    CaptureFilename,
    CaptureFormat,
    SessionCustomDirectory,
    SessionMaxShapesPerFrame,
    SessionMaxFileSizeMb,
    SessionAutoCompressThresholdKb,
    SessionBackupRetention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripletField {
    DrawingColorRgb,
    BoardWhiteboard,
    BoardBlackboard,
    BoardWhiteboardPen,
    BoardBlackboardPen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadField {
    StatusBarBg,
    StatusBarText,
    HelpBg,
    HelpBorder,
    HelpText,
    HighlightFill,
    HighlightOutline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStorageModeOption {
    Auto,
    Config,
    Custom,
}

impl SessionStorageModeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Auto, Self::Config, Self::Custom]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Config => "Config directory",
            Self::Custom => "Custom directory",
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_mode(&self) -> SessionStorageMode {
        match self {
            Self::Auto => SessionStorageMode::Auto,
            Self::Config => SessionStorageMode::Config,
            Self::Custom => SessionStorageMode::Custom,
        }
    }

    pub fn from_mode(mode: SessionStorageMode) -> Self {
        match mode {
            SessionStorageMode::Auto => Self::Auto,
            SessionStorageMode::Config => Self::Config,
            SessionStorageMode::Custom => Self::Custom,
        }
    }
}

impl std::fmt::Display for SessionStorageModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionCompressionOption {
    Auto,
    On,
    Off,
}

impl SessionCompressionOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Auto, Self::On, Self::Off]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::On => "On",
            Self::Off => "Off",
        }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn to_compression(&self) -> SessionCompression {
        match self {
            Self::Auto => SessionCompression::Auto,
            Self::On => SessionCompression::On,
            Self::Off => SessionCompression::Off,
        }
    }

    pub fn from_compression(mode: SessionCompression) -> Self {
        match mode {
            SessionCompression::Auto => Self::Auto,
            SessionCompression::On => Self::On,
            SessionCompression::Off => Self::Off,
        }
    }
}

impl std::fmt::Display for SessionCompressionOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
