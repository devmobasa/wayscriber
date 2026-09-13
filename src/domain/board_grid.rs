//! Board-paper geometry, independent of rendering and serialization.

pub const BOARD_GRID_MIN_SPACING: u16 = 8;
pub const BOARD_GRID_MAX_SPACING: u16 = 200;
pub const BOARD_GRID_DEFAULT_SPACING: u16 = 40;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BoardGridKind {
    #[default]
    None,
    Cartesian,
    Isometric,
    IsometricDots,
}

impl BoardGridKind {
    pub const ALL: [Self; 4] = [
        Self::None,
        Self::Cartesian,
        Self::Isometric,
        Self::IsometricDots,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Cartesian => "Cartesian",
            Self::Isometric => "Isometric lines",
            Self::IsometricDots => "Isometric dots",
        }
    }
}

/// A normalized grid. Spacing is a square/triangle side in logical board pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoardGrid {
    pub kind: BoardGridKind,
    spacing: u16,
}

impl Default for BoardGrid {
    fn default() -> Self {
        Self {
            kind: BoardGridKind::None,
            spacing: BOARD_GRID_DEFAULT_SPACING,
        }
    }
}

impl BoardGrid {
    pub fn new(kind: BoardGridKind, spacing: i64) -> Self {
        Self {
            kind,
            spacing: spacing.clamp(
                i64::from(BOARD_GRID_MIN_SPACING),
                i64::from(BOARD_GRID_MAX_SPACING),
            ) as u16,
        }
    }

    pub fn spacing(self) -> u16 {
        self.spacing
    }

    /// Disable decoration without discarding its spacing.
    pub fn disabled(self) -> Self {
        Self {
            kind: BoardGridKind::None,
            ..self
        }
    }
}
