use crate::draw::shape::Shape;
use crate::util::Rect;
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

/// Geometry snapshot for image-only move/resize undo without duplicating bytes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageBoundsSnapshot {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub locked: bool,
}

impl ImageBoundsSnapshot {
    pub fn from_shape(shape: &Shape, locked: bool) -> Option<Self> {
        match shape {
            Shape::Image { x, y, w, h, .. } => Some(Self {
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                locked,
            }),
            _ => None,
        }
    }

    pub fn bounding_box(&self) -> Option<Rect> {
        let min_x = if self.w < 0 {
            self.x.saturating_add(self.w)
        } else {
            self.x
        };
        let min_y = if self.h < 0 {
            self.y.saturating_add(self.h)
        } else {
            self.y
        };
        Rect::new(
            min_x,
            min_y,
            self.w.saturating_abs().max(1),
            self.h.saturating_abs().max(1),
        )
    }
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
    ModifyImageBounds {
        shape_id: ShapeId,
        before: ImageBoundsSnapshot,
        after: ImageBoundsSnapshot,
    },
    Reorder {
        shape_id: ShapeId,
        from: usize,
        to: usize,
    },
    Compound(Vec<UndoAction>),
}

impl UndoAction {
    pub fn modify_from_snapshots(
        shape_id: ShapeId,
        before: ShapeSnapshot,
        after: ShapeSnapshot,
    ) -> Self {
        if image_payload_unchanged(&before.shape, &after.shape)
            && let (Some(before), Some(after)) = (
                ImageBoundsSnapshot::from_shape(&before.shape, before.locked),
                ImageBoundsSnapshot::from_shape(&after.shape, after.locked),
            )
        {
            return UndoAction::ModifyImageBounds {
                shape_id,
                before,
                after,
            };
        }

        UndoAction::Modify {
            shape_id,
            before,
            after,
        }
    }
}

fn image_payload_unchanged(before: &Shape, after: &Shape) -> bool {
    match (before, after) {
        (Shape::Image { data: before, .. }, Shape::Image { data: after, .. }) => before == after,
        _ => false,
    }
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
