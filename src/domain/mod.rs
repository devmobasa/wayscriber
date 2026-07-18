//! Stable, dependency-light application value types.
//!
//! This module owns identities shared by configuration, input, drawing, and UI
//! layers. Runtime policy, rendering, persistence, and mutable state belong to
//! their respective higher-level modules.

mod action;
mod board;
pub mod color;
mod tool;

pub use action::Action;
pub use board::{
    BOARD_ID_BLACKBOARD, BOARD_ID_TRANSPARENT, BOARD_ID_WHITEBOARD, BoardBackground, BoardSpec,
};
pub use color::Color;
pub use tool::{DragBindableTool, DragTool, EraserMode, Tool};

#[cfg(test)]
mod tests;
