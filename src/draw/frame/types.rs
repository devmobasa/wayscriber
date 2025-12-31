use crate::draw::shape::Shape;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique identifier for a drawn shape within a frame.
pub type ShapeId = u64;

/// Maximum allowed compound nesting depth in persisted history.
pub const MAX_COMPOUND_DEPTH: usize = 16;

/// A shape stored in a frame with additional metadata.
#[derive(Clone, Debug)]
pub struct DrawnShape {
    pub id: ShapeId,
    pub shape: Shape,
    pub created_at: u64,
    pub locked: bool,
}

impl DrawnShape {
    pub(super) fn new(id: ShapeId, shape: Shape) -> Self {
        Self {
            id,
            shape,
            created_at: current_timestamp_ms(),
            locked: false,
        }
    }

    pub(super) fn with_metadata(id: ShapeId, shape: Shape, created_at: u64, locked: bool) -> Self {
        Self {
            id,
            shape,
            created_at,
            locked,
        }
    }
}

/// Snapshot of a shape used for undo/redo of modifications.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShapeSnapshot {
    pub shape: Shape,
    pub locked: bool,
}

/// Undoable actions stored in the frame history.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum UndoAction {
    Create {
        shapes: Vec<(usize, DrawnShape)>,
    },
    Delete {
        shapes: Vec<(usize, DrawnShape)>,
    },
    Modify {
        shape_id: ShapeId,
        before: ShapeSnapshot,
        after: ShapeSnapshot,
    },
    Reorder {
        shape_id: ShapeId,
        from: usize,
        to: usize,
    },
    Compound(Vec<UndoAction>),
}

/// Result of trimming or validating undo/redo history.
#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryTrimStats {
    pub undo_removed: usize,
    pub redo_removed: usize,
}

impl HistoryTrimStats {
    pub fn is_empty(&self) -> bool {
        self.undo_removed == 0 && self.redo_removed == 0
    }

    pub(super) fn add_undo(&mut self, count: usize) {
        self.undo_removed = self.undo_removed.saturating_add(count);
    }

    pub(super) fn add_redo(&mut self, count: usize) {
        self.redo_removed = self.redo_removed.saturating_add(count);
    }
}

pub(super) fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}
