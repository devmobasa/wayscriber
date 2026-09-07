//! Frame container for managing collections of shapes with undo/redo support.

mod core;
mod frame_storage;
mod history;
mod serde;
mod shapes;
mod types;

#[cfg(test)]
mod tests;

pub use core::Frame;
pub use shapes::FrameShapes;
#[allow(unused_imports)]
pub use types::{
    DrawnShape, HistoryTrimStats, ImageBoundsSnapshot, MAX_COMPOUND_DEPTH, ShapeId, ShapeSnapshot,
    UndoAction,
};
