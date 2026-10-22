use std::fmt;
use std::fmt::{Display, Formatter};

use wayscriber::i18n::tr;

use wayscriber::config::{
    PdfFitMode, PdfLabelContentMode, PdfLabelPosition, PdfOrientation, PdfPageSize,
    PdfTransparentBackground,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfPageSizeOption {
    Viewport,
    A4,
    Letter,
    Custom,
}

impl PdfPageSizeOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Viewport, Self::A4, Self::Letter, Self::Custom]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Viewport => tr("Viewport"),
            Self::A4 => "A4",
            Self::Letter => tr("Letter"),
            Self::Custom => tr("Custom"),
        }
    }

    pub fn to_config(self) -> PdfPageSize {
        match self {
            Self::Viewport => PdfPageSize::Viewport,
            Self::A4 => PdfPageSize::A4,
            Self::Letter => PdfPageSize::Letter,
            Self::Custom => PdfPageSize::Custom,
        }
    }

    pub fn from_config(value: PdfPageSize) -> Self {
        match value {
            PdfPageSize::Viewport => Self::Viewport,
            PdfPageSize::A4 => Self::A4,
            PdfPageSize::Letter => Self::Letter,
            PdfPageSize::Custom => Self::Custom,
        }
    }
}

impl Display for PdfPageSizeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfOrientationOption {
    Auto,
    Portrait,
    Landscape,
}

impl PdfOrientationOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Auto, Self::Portrait, Self::Landscape]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => tr("Auto"),
            Self::Portrait => tr("Portrait"),
            Self::Landscape => tr("Landscape"),
        }
    }

    pub fn to_config(self) -> PdfOrientation {
        match self {
            Self::Auto => PdfOrientation::Auto,
            Self::Portrait => PdfOrientation::Portrait,
            Self::Landscape => PdfOrientation::Landscape,
        }
    }

    pub fn from_config(value: PdfOrientation) -> Self {
        match value {
            PdfOrientation::Auto => Self::Auto,
            PdfOrientation::Portrait => Self::Portrait,
            PdfOrientation::Landscape => Self::Landscape,
        }
    }
}

impl Display for PdfOrientationOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfFitModeOption {
    Viewport,
    FitViewportToPage,
    FitContentToPage,
}

impl PdfFitModeOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::Viewport,
            Self::FitViewportToPage,
            Self::FitContentToPage,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Viewport => tr("Viewport"),
            Self::FitViewportToPage => tr("Fit viewport"),
            Self::FitContentToPage => tr("Fit content"),
        }
    }

    pub fn to_config(self) -> PdfFitMode {
        match self {
            Self::Viewport => PdfFitMode::Viewport,
            Self::FitViewportToPage => PdfFitMode::FitViewportToPage,
            Self::FitContentToPage => PdfFitMode::FitContentToPage,
        }
    }

    pub fn from_config(value: PdfFitMode) -> Self {
        match value {
            PdfFitMode::Viewport => Self::Viewport,
            PdfFitMode::FitViewportToPage => Self::FitViewportToPage,
            PdfFitMode::FitContentToPage => Self::FitContentToPage,
        }
    }
}

impl Display for PdfFitModeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfTransparentBackgroundOption {
    None,
    Desktop,
}

impl PdfTransparentBackgroundOption {
    pub fn list() -> Vec<Self> {
        vec![Self::None, Self::Desktop]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None => tr("None"),
            Self::Desktop => tr("Live desktop"),
        }
    }

    pub fn to_config(self) -> PdfTransparentBackground {
        match self {
            Self::None => PdfTransparentBackground::None,
            Self::Desktop => PdfTransparentBackground::Desktop,
        }
    }

    pub fn from_config(value: PdfTransparentBackground) -> Self {
        match value {
            PdfTransparentBackground::None => Self::None,
            PdfTransparentBackground::Desktop => Self::Desktop,
        }
    }
}

impl Display for PdfTransparentBackgroundOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfLabelPositionOption {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    BottomCenter,
}

impl PdfLabelPositionOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::TopLeft,
            Self::TopRight,
            Self::BottomLeft,
            Self::BottomRight,
            Self::BottomCenter,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::TopLeft => tr("Top left"),
            Self::TopRight => tr("Top right"),
            Self::BottomLeft => tr("Bottom left"),
            Self::BottomRight => tr("Bottom right"),
            Self::BottomCenter => tr("Bottom center"),
        }
    }

    pub fn to_config(self) -> PdfLabelPosition {
        match self {
            Self::TopLeft => PdfLabelPosition::TopLeft,
            Self::TopRight => PdfLabelPosition::TopRight,
            Self::BottomLeft => PdfLabelPosition::BottomLeft,
            Self::BottomRight => PdfLabelPosition::BottomRight,
            Self::BottomCenter => PdfLabelPosition::BottomCenter,
        }
    }

    pub fn from_config(value: PdfLabelPosition) -> Self {
        match value {
            PdfLabelPosition::TopLeft => Self::TopLeft,
            PdfLabelPosition::TopRight => Self::TopRight,
            PdfLabelPosition::BottomLeft => Self::BottomLeft,
            PdfLabelPosition::BottomRight => Self::BottomRight,
            PdfLabelPosition::BottomCenter => Self::BottomCenter,
        }
    }
}

impl Display for PdfLabelPositionOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfLabelContentModeOption {
    CustomTemplate,
    BoardAndPage,
    DocumentPage,
    BoardName,
    PageName,
}

impl PdfLabelContentModeOption {
    pub fn list() -> Vec<Self> {
        vec![
            Self::CustomTemplate,
            Self::BoardAndPage,
            Self::DocumentPage,
            Self::BoardName,
            Self::PageName,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CustomTemplate => tr("Custom template"),
            Self::BoardAndPage => tr("Board and page"),
            Self::DocumentPage => tr("Document page"),
            Self::BoardName => tr("Board name"),
            Self::PageName => tr("Page name"),
        }
    }

    pub fn to_config(self) -> PdfLabelContentMode {
        match self {
            Self::CustomTemplate => PdfLabelContentMode::CustomTemplate,
            Self::BoardAndPage => PdfLabelContentMode::BoardAndPage,
            Self::DocumentPage => PdfLabelContentMode::DocumentPage,
            Self::BoardName => PdfLabelContentMode::BoardName,
            Self::PageName => PdfLabelContentMode::PageName,
        }
    }

    pub fn from_config(value: PdfLabelContentMode) -> Self {
        match value {
            PdfLabelContentMode::CustomTemplate => Self::CustomTemplate,
            PdfLabelContentMode::BoardAndPage => Self::BoardAndPage,
            PdfLabelContentMode::DocumentPage => Self::DocumentPage,
            PdfLabelContentMode::BoardName => Self::BoardName,
            PdfLabelContentMode::PageName => Self::PageName,
        }
    }
}

impl Display for PdfLabelContentModeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
