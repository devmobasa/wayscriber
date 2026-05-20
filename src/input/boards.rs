mod color;
mod core;
mod identity;
mod mapping;
mod naming;
mod operations;
#[cfg(test)]
mod tests;

use crate::draw::{BoardPages, Color};

#[allow(unused_imports)]
pub use color::{
    board_color_from_config, board_color_to_config, clamp_board_rgb, runtime_contrast_pen_color,
    runtime_contrast_pen_rgb,
};
#[allow(unused_imports)]
pub use identity::{
    BoardIdChangeSet, BoardIdentityGeneration, BoundaryBoardId, BoundaryBoardIdSet,
};
pub use operations::{
    BoardDeleteConfirmation, BoardDeleteOutcome, BoardDeleteRejection, BoardDeleteRequest,
    BoardDeleteTarget, BoardRestoreOutcome, BoardRestoreRejection, BoardRestoreRequest,
    PageDeleteBoardTarget, PageDeleteConfirmation, PageDeleteOutcome, PageDeleteRequest,
    PageDeleteTarget, PageOperationRejection, PageRestoreOutcome, PageRestorePlacement,
    PageRestoreRejection, PageRestoreRequest,
};

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

#[derive(Debug)]
pub struct BoardManager {
    boards: Vec<BoardState>,
    active_index: usize,
    max_count: usize,
    auto_create: bool,
    show_badge: bool,
    pan_enabled: bool,
    show_pan_badge: bool,
    persist_customizations: bool,
    default_board_id: String,
    template: BoardSpec,
    identity_generation: BoardIdentityGeneration,
}

impl Clone for BoardManager {
    fn clone(&self) -> Self {
        Self {
            boards: self.boards.clone(),
            active_index: self.active_index,
            max_count: self.max_count,
            auto_create: self.auto_create,
            show_badge: self.show_badge,
            pan_enabled: self.pan_enabled,
            show_pan_badge: self.show_pan_badge,
            persist_customizations: self.persist_customizations,
            default_board_id: self.default_board_id.clone(),
            template: self.template.clone(),
            identity_generation: BoardIdentityGeneration::fresh(),
        }
    }
}
