mod mapping;
mod validation;

use super::super::color::ColorTripletInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardBackgroundOption {
    Transparent,
    Color,
}

impl BoardBackgroundOption {
    pub fn list() -> Vec<Self> {
        vec![Self::Transparent, Self::Color]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Transparent => "Transparent",
            Self::Color => "Solid color",
        }
    }
}

impl std::fmt::Display for BoardBackgroundOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardItemTextField {
    Id,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardItemToggleField {
    AutoAdjustPen,
    Persist,
    Pinned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OptionalTripletInput {
    pub enabled: bool,
    pub color: ColorTripletInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardItemDraft {
    pub id: String,
    pub name: String,
    pub background_kind: BoardBackgroundOption,
    pub background_color: ColorTripletInput,
    pub default_pen_color: OptionalTripletInput,
    pub auto_adjust_pen: bool,
    pub persist: bool,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardsDraft {
    pub max_count: String,
    pub auto_create: bool,
    pub show_board_badge: bool,
    pub persist_customizations: bool,
    pub default_board: String,
    pub items: Vec<BoardItemDraft>,
}
