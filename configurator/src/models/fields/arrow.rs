use wayscriber::draw::ArrowStyle;

/// Startup shape of the arrow tool, as a combo-row choice.
///
/// Wraps `ArrowStyle` rather than binding the core enum directly so the
/// configurator owns its own ordering and its own labels — the combo row lists
/// these in declaration order, which is not something the drawing code should
/// have to preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowStyleOption {
    Standard,
    Pointy,
    Curved,
    Double,
}

impl ArrowStyleOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Standard, Self::Pointy, Self::Curved, Self::Double]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Pointy => "Pointy",
            Self::Curved => "Curved",
            Self::Double => "Double",
        }
    }

    pub fn to_style(self) -> ArrowStyle {
        match self {
            Self::Standard => ArrowStyle::Standard,
            Self::Pointy => ArrowStyle::Pointy,
            Self::Curved => ArrowStyle::Curved,
            Self::Double => ArrowStyle::Double,
        }
    }

    pub fn from_style(style: ArrowStyle) -> Self {
        match style {
            ArrowStyle::Standard => Self::Standard,
            ArrowStyle::Pointy => Self::Pointy,
            ArrowStyle::Curved => Self::Curved,
            ArrowStyle::Double => Self::Double,
        }
    }
}

impl std::fmt::Display for ArrowStyleOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
