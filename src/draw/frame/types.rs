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

    pub(crate) fn with_metadata(id: ShapeId, shape: Shape, created_at: u64, locked: bool) -> Self {
        Self {
            id,
            shape,
            created_at,
            locked,
        }
    }

    /// Bounding box of the current shape.
    ///
    /// Derived geometry is computed from the shape that this value owns. The
    /// render owner may cache frame-level culling data; the domain value does
    /// not mutate itself through a shared reference.
    pub fn bounding_box(&self) -> Option<Rect> {
        self.shape.bounding_box()
    }

    /// Replaces the shape, keeping the bounds cache consistent.
    pub fn set_shape(&mut self, shape: Shape) {
        self.shape = shape;
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
    Compound {
        actions: Vec<UndoAction>,
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Color;

    fn line(x1: i32, y1: i32, x2: i32, y2: i32) -> Shape {
        Shape::Line {
            x1,
            y1,
            x2,
            y2,
            color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            thick: 2.0,
        }
    }

    #[test]
    fn bounding_box_is_derived_and_stable() {
        let drawn = DrawnShape::with_metadata(1, line(0, 0, 10, 10), 0, false);
        let first = drawn.bounding_box();
        let second = drawn.bounding_box();
        assert_eq!(first, second);
        assert_eq!(first, drawn.shape.bounding_box());
    }

    #[test]
    fn set_shape_updates_derived_bounds() {
        let mut drawn = DrawnShape::with_metadata(1, line(0, 0, 10, 10), 0, false);
        let before = drawn.bounding_box().expect("line has bounds");
        drawn.set_shape(line(100, 100, 150, 150));
        let after = drawn.bounding_box().expect("moved line has bounds");
        assert_ne!(before, after);
        assert_eq!(Some(after), drawn.shape.bounding_box());
    }

    #[test]
    fn in_place_mutation_is_reflected_without_interior_cache_state() {
        let mut drawn = DrawnShape::with_metadata(1, line(0, 0, 10, 10), 0, false);
        let _ = drawn.bounding_box();
        if let Shape::Line { x1, x2, .. } = &mut drawn.shape {
            *x1 += 500;
            *x2 += 500;
        }
        assert_eq!(drawn.bounding_box(), drawn.shape.bounding_box());
    }
}
