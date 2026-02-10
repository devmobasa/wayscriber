mod core;
mod mapping;
mod naming;

use crate::draw::{BoardPages, Color};

pub const BOARD_ID_TRANSPARENT: &str = "transparent";
pub const BOARD_ID_WHITEBOARD: &str = "whiteboard";
pub const BOARD_ID_BLACKBOARD: &str = "blackboard";

#[derive(Debug, Clone)]
pub enum BoardBackground {
    Transparent,
    Solid(Color),
}

impl BoardBackground {
    pub fn is_transparent(&self) -> bool {
        matches!(self, BoardBackground::Transparent)
    }
}

#[derive(Debug, Clone)]
pub struct BoardSpec {
    pub id: String,
    pub name: String,
    pub background: BoardBackground,
    pub default_pen_color: Option<Color>,
    pub auto_adjust_pen: bool,
    pub persist: bool,
    pub pinned: bool,
}

#[derive(Debug, Clone)]
pub struct BoardState {
    pub spec: BoardSpec,
    pub pages: BoardPages,
}

impl BoardState {
    pub fn new(spec: BoardSpec) -> Self {
        Self {
            spec,
            pages: BoardPages::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BoardManager {
    boards: Vec<BoardState>,
    active_index: usize,
    max_count: usize,
    auto_create: bool,
    show_badge: bool,
    persist_customizations: bool,
    default_board_id: String,
    template: BoardSpec,
}
