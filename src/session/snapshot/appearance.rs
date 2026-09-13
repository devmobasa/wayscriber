use crate::config::BoardGridConfig;
use crate::domain::{BoardBackground, Color};
use crate::input::boards::{BoardAppearance, BoardPenOrigin, BoardState};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardAppearanceSnapshot {
    pub background: Option<Color>,
    pub grid: BoardGridConfig,
    pub default_pen_color: Option<Color>,
    pub auto_adjust_pen: bool,
    pub pen_origin: BoardPenOrigin,
    pub explicit: bool,
}

impl BoardAppearanceSnapshot {
    pub(crate) fn capture(board: &BoardState) -> Self {
        Self {
            background: match board.spec.background {
                BoardBackground::Transparent => None,
                BoardBackground::Solid(color) => Some(color),
            },
            grid: board.spec.grid.into(),
            default_pen_color: board.spec.default_pen_color,
            auto_adjust_pen: board.spec.auto_adjust_pen,
            pen_origin: board.pen_origin,
            explicit: board.appearance_explicit,
        }
    }

    pub(crate) fn is_valid(&self) -> bool {
        fn valid(color: Color) -> bool {
            [color.r, color.g, color.b, color.a]
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
        }
        self.background.is_none_or(valid)
            && self.default_pen_color.is_none_or(valid)
            && (8..=200).contains(&self.grid.spacing)
    }

    pub(crate) fn apply(&self, board: &mut BoardState) {
        if !self.is_valid() {
            return;
        }
        // The reserved overlay identity must stay transparent.
        let background = if board.spec.id == crate::domain::BOARD_ID_TRANSPARENT {
            BoardBackground::Transparent
        } else {
            self.background
                .map_or(BoardBackground::Transparent, BoardBackground::Solid)
        };
        BoardAppearance {
            background,
            grid: self.grid.into(),
            default_pen_color: self.default_pen_color,
            auto_adjust_pen: self.auto_adjust_pen,
        }
        .apply_to(&mut board.spec);
        board.pen_origin = self.pen_origin;
        board.appearance_explicit = self.explicit;
    }
}

pub(super) fn deserialize_appearance<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<BoardAppearanceSnapshot>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(d)?;
    let Some(value) = value else {
        return Ok(None);
    };
    if !value.as_object().is_some_and(|fields| {
        fields.contains_key("background") && fields.contains_key("default_pen_color")
    }) {
        log::warn!("Ignoring incomplete saved board appearance; using configured paper");
        return Ok(None);
    }
    match serde_json::from_value::<BoardAppearanceSnapshot>(value) {
        Ok(appearance) if appearance.is_valid() => Ok(Some(appearance)),
        _ => {
            log::warn!("Ignoring invalid saved board appearance; using configured paper");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests;
