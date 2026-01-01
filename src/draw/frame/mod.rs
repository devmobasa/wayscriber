//! Frame container for managing collections of shapes with undo/redo support.

mod core;
mod frame_storage;
mod history;
mod serde;
mod types;

#[cfg(test)]
mod tests;

pub use core::Frame;
#[allow(unused_imports)]
pub use types::{
    DrawnShape, HistoryTrimStats, MAX_COMPOUND_DEPTH, ShapeId, ShapeSnapshot, UndoAction,
};
