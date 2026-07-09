//! Config-facing input and tool model types.

pub mod boards;
pub mod state;
pub mod tool;

pub use boards::{
    BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoundaryBoardIdSet,
    clamp_board_rgb,
};
pub use tool::{DragBindableTool, DragTool, EraserMode, PerToolDrawingSettings, Tool};
