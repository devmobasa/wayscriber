use std::fmt;
use std::fmt::{Display, Formatter};

use wayscriber::i18n::tr;

use wayscriber::config::StatusPosition;

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
            StatusPositionOption::TopLeft => tr("Top Left"),
            StatusPositionOption::TopRight => tr("Top Right"),
            StatusPositionOption::BottomLeft => tr("Bottom Left"),
            StatusPositionOption::BottomRight => tr("Bottom Right"),
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

impl Display for StatusPositionOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
