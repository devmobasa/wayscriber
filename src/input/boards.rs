mod color;
mod core;
mod identity;
mod mapping;
mod naming;
mod operations;
#[cfg(test)]
mod tests;

use crate::draw::BoardPages;

pub use crate::domain::{
    BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardBackground, BoardSpec,
};

#[allow(unused_imports)]
pub use color::{
    board_color_from_config, board_color_to_config, clamp_board_rgb, runtime_contrast_pen_color,
    runtime_contrast_pen_rgb,
};
#[allow(unused_imports)]
pub use identity::{
    BoardIdChangeSet, BoardIdentityGeneration, BoundaryBoardId, BoundaryBoardIdSet,
};
pub(crate) use mapping::{BoardConfigChange, PendingBoardConfigUpdate};
pub use operations::{
    BoardDeleteConfirmation, BoardDeleteOutcome, BoardDeleteRejection, BoardDeleteRequest,
    BoardDeleteTarget, BoardRestoreOutcome, BoardRestoreRejection, BoardRestoreRequest,
    PageDeleteBoardTarget, PageDeleteConfirmation, PageDeleteOutcome, PageDeleteRequest,
    PageDeleteTarget, PageOperationRejection, PageRestoreOutcome, PageRestorePlacement,
    PageRestoreRejection, PageRestoreRequest,
};

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

impl BoardManager {
    pub(crate) fn clone_preserving_identity_generation(&self) -> Self {
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
            identity_generation: self.identity_generation,
        }
    }
}
