use super::{BoardBackground, BoardManager, BoardSpec, BoardState};
use crate::domain::{BoardGrid, Color};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoardPenOrigin {
    #[default]
    Configured,
    RuntimeContrast,
}

/// The reference paper and its board-entry pen policy, independent of drawing history.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardAppearance {
    pub background: BoardBackground,
    pub grid: BoardGrid,
    pub default_pen_color: Option<Color>,
    pub auto_adjust_pen: bool,
}

impl BoardAppearance {
    pub fn from_spec(spec: &BoardSpec) -> Self {
        Self {
            background: spec.background.clone(),
            grid: spec.grid,
            default_pen_color: spec.default_pen_color,
            auto_adjust_pen: spec.auto_adjust_pen,
        }
    }

    pub fn apply_to(&self, spec: &mut BoardSpec) {
        spec.background = self.background.clone();
        spec.grid = if self.background.is_transparent() {
            self.grid.disabled()
        } else {
            self.grid
        };
        spec.default_pen_color = self.default_pen_color;
        spec.auto_adjust_pen = self.auto_adjust_pen;
    }
}

impl BoardState {
    /// Reset from the immutable configured/template seed, never from a prior session.
    pub(crate) fn reset_appearance(&mut self) {
        self.configured_appearance.apply_to(&mut self.spec);
        self.appearance_explicit = false;
        self.pen_origin = BoardPenOrigin::Configured;
    }
}

impl BoardManager {
    pub(crate) fn reset_appearances(&mut self) {
        for board in &mut self.boards {
            board.reset_appearance();
        }
    }
}
