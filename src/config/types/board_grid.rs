use serde::{Deserialize, Serialize};

use crate::domain::{BOARD_GRID_DEFAULT_SPACING, BoardGrid, BoardGridKind};

#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoardGridKindConfig {
    #[default]
    None,
    Cartesian,
    Isometric,
    IsometricDots,
}

impl From<BoardGridKindConfig> for BoardGridKind {
    fn from(value: BoardGridKindConfig) -> Self {
        match value {
            BoardGridKindConfig::None => Self::None,
            BoardGridKindConfig::Cartesian => Self::Cartesian,
            BoardGridKindConfig::Isometric => Self::Isometric,
            BoardGridKindConfig::IsometricDots => Self::IsometricDots,
        }
    }
}

impl From<BoardGridKind> for BoardGridKindConfig {
    fn from(value: BoardGridKind) -> Self {
        match value {
            BoardGridKind::None => Self::None,
            BoardGridKind::Cartesian => Self::Cartesian,
            BoardGridKind::Isometric => Self::Isometric,
            BoardGridKind::IsometricDots => Self::IsometricDots,
        }
    }
}

/// Paper decoration for a solid board. Config validation warns before clamping.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardGridConfig {
    pub kind: BoardGridKindConfig,
    /// Square/triangle side in logical board pixels, from 8 through 200.
    #[cfg_attr(feature = "config-schema", schemars(range(min = 8, max = 200)))]
    pub spacing: i64,
}

impl Default for BoardGridConfig {
    fn default() -> Self {
        Self {
            kind: BoardGridKindConfig::None,
            spacing: i64::from(BOARD_GRID_DEFAULT_SPACING),
        }
    }
}

impl From<BoardGridConfig> for BoardGrid {
    fn from(value: BoardGridConfig) -> Self {
        Self::new(value.kind.into(), value.spacing)
    }
}

impl From<BoardGrid> for BoardGridConfig {
    fn from(value: BoardGrid) -> Self {
        Self {
            kind: value.kind.into(),
            spacing: i64::from(value.spacing()),
        }
    }
}
