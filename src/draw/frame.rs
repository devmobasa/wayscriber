//! Frame container for managing collections of shapes with undo/redo support.

use super::shape::Shape;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique identifier for a drawn shape within a frame.
pub type ShapeId = u64;

/// A shape stored in a frame with additional metadata.
#[derive(Clone, Debug)]
pub struct DrawnShape {
    pub id: ShapeId,
    pub shape: Shape,
    pub created_at: u64,
    pub locked: bool,
}

impl DrawnShape {
    fn new(id: ShapeId, shape: Shape) -> Self {
        Self {
            id,
            shape,
            created_at: current_timestamp_ms(),
            locked: false,
        }
    }

    fn with_metadata(id: ShapeId, shape: Shape, created_at: u64, locked: bool) -> Self {
        Self {
            id,
            shape,
            created_at,
            locked,
        }
    }
}

/// Snapshot of a shape used for undo/redo of modifications.
#[derive(Clone, Debug)]
pub struct ShapeSnapshot {
    pub shape: Shape,
    pub locked: bool,
}

/// Undoable actions stored in the frame history.
#[derive(Clone, Debug)]
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

#[derive(Debug, Clone, Serialize)]
pub struct Frame {
    #[serde(with = "frame_storage")]
    pub shapes: Vec<DrawnShape>,
    #[serde(skip)]
    undo_stack: Vec<UndoAction>,
    #[serde(skip)]
    redo_stack: Vec<UndoAction>,
    #[serde(skip)]
    next_shape_id: ShapeId,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    /// Creates a new empty frame.
    pub fn new() -> Self {
        Self {
            shapes: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_shape_id: 1,
        }
    }

    /// Clears all shapes and history from the frame.
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.next_shape_id = 1;
    }

    #[allow(dead_code)]
    /// Returns the number of shapes in the frame.
    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    #[allow(dead_code)]
    /// Returns true if the frame contains no shapes.
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// Adds a shape at the end of the stack and returns its identifier.
    pub fn add_shape(&mut self, shape: Shape) -> ShapeId {
        let index = self.shapes.len();
        let id = self.insert_new_shape(index, shape);
        self.redo_stack.clear();
        id
    }

    /// Attempts to add a shape respecting a maximum count.
    ///
    /// Returns true if the shape was added.
    pub fn try_add_shape(&mut self, shape: Shape, max: usize) -> bool {
        self.try_add_shape_with_id(shape, max).is_some()
    }

    /// Attempts to add a shape and returns the identifier on success.
    pub fn try_add_shape_with_id(&mut self, shape: Shape, max: usize) -> Option<ShapeId> {
        if max > 0 && self.shapes.len() >= max {
            return None;
        }
        Some(self.add_shape(shape))
    }

    #[allow(dead_code)]
    /// Inserts a shape at the given index and returns its identifier.
    pub fn insert_shape_at(&mut self, index: usize, shape: Shape) -> ShapeId {
        let index = index.min(self.shapes.len());
        let id = self.insert_new_shape(index, shape);
        self.redo_stack.clear();
        id
    }

    /// Records an undoable action, enforcing a stack limit.
    pub fn push_undo_action(&mut self, action: UndoAction, limit: usize) {
        self.undo_stack.push(action);
        if limit > 0 && self.undo_stack.len() > limit {
            let overflow = self.undo_stack.len() - limit;
            self.undo_stack.drain(0..overflow);
        }
        self.redo_stack.clear();
    }

    /// Undoes the most recent action, returning it for external bookkeeping.
    pub fn undo_last(&mut self) -> Option<UndoAction> {
        let action = self.undo_stack.pop()?;
        self.apply_inverse(&action);
        self.redo_stack.push(action.clone());
        Some(action)
    }

    /// Redoes the most recently undone action.
    pub fn redo_last(&mut self) -> Option<UndoAction> {
        let action = self.redo_stack.pop()?;
        self.apply_action(&action);
        self.undo_stack.push(action.clone());
        Some(action)
    }

    #[allow(dead_code)]
    /// Legacy helper used by existing code paths to undo an action and retrieve a representative shape.
    pub fn undo(&mut self) -> Option<Shape> {
        let action = self.undo_last()?;
        Self::primary_shape_for_undo(&action)
    }

    #[allow(dead_code)]
    /// Legacy helper used by existing code paths to redo an action and retrieve a representative shape.
    pub fn redo(&mut self) -> Option<Shape> {
        let action = self.redo_last()?;
        Self::primary_shape_for_redo(&action)
    }

    #[allow(dead_code)]
    /// Returns a reference to the undo stack (for testing).
    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.len()
    }

    #[allow(dead_code)]
    /// Returns a reference to the redo stack length (for testing).
    pub fn redo_stack_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Finds the index of a shape by id.
    pub fn find_index(&self, id: ShapeId) -> Option<usize> {
        self.shapes.iter().position(|shape| shape.id == id)
    }

    /// Returns a reference to a shape by id.
    pub fn shape(&self, id: ShapeId) -> Option<&DrawnShape> {
        self.find_index(id).map(|i| &self.shapes[i])
    }

    /// Returns a mutable reference to a shape by id.
    pub fn shape_mut(&mut self, id: ShapeId) -> Option<&mut DrawnShape> {
        self.find_index(id).map(|i| &mut self.shapes[i])
    }

    /// Removes a shape by id, returning its index and data.
    pub fn remove_shape_by_id(&mut self, id: ShapeId) -> Option<(usize, DrawnShape)> {
        let index = self.find_index(id)?;
        Some((index, self.shapes.remove(index)))
    }

    /// Moves a shape from one index to another.
    pub fn move_shape(&mut self, from: usize, to: usize) -> Option<()> {
        if from >= self.shapes.len() || to >= self.shapes.len() {
            return None;
        }
        if from == to {
            return Some(());
        }
        let shape = self.shapes.remove(from);
        let mut insert_index = to.min(self.shapes.len());
        if from < to && insert_index > 0 {
            insert_index -= 1;
        }
        self.shapes.insert(insert_index, shape);
        Some(())
    }

    fn insert_new_shape(&mut self, index: usize, shape: Shape) -> ShapeId {
        let id = self.generate_id();
        let drawn = DrawnShape::new(id, shape);
        self.insert_existing(index, drawn);
        id
    }

    fn insert_existing(&mut self, index: usize, drawn: DrawnShape) {
        self.mark_id_used(drawn.id);
        self.shapes.insert(index, drawn);
    }

    fn apply_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } => {
                for (offset, (index, shape)) in shapes.iter().enumerate() {
                    self.insert_existing(index + offset, shape.clone());
                }
            }
            UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.remove_shape_by_id(shape.id);
                }
            }
            UndoAction::Modify {
                shape_id, after, ..
            } => {
                if let Some(target) = self.shape_mut(*shape_id) {
                    target.shape = after.shape.clone();
                    target.locked = after.locked;
                }
            }
            UndoAction::Reorder {
                shape_id,
                from: _,
                to,
            } => {
                self.move_shape_to(*shape_id, *to);
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.apply_action(action);
                }
            }
        }
    }

    fn apply_inverse(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } => {
                for (_, shape) in shapes.iter().rev() {
                    self.remove_shape_by_id(shape.id);
                }
            }
            UndoAction::Delete { shapes } => {
                for (offset, (index, shape)) in shapes.iter().enumerate() {
                    self.insert_existing(index + offset, shape.clone());
                }
            }
            UndoAction::Modify {
                shape_id, before, ..
            } => {
                if let Some(target) = self.shape_mut(*shape_id) {
                    target.shape = before.shape.clone();
                    target.locked = before.locked;
                }
            }
            UndoAction::Reorder { shape_id, from, .. } => {
                self.move_shape_to(*shape_id, *from);
            }
            UndoAction::Compound(actions) => {
                for action in actions.iter().rev() {
                    self.apply_inverse(action);
                }
            }
        }
    }

    fn move_shape_to(&mut self, shape_id: ShapeId, target: usize) {
        if let Some(index) = self.find_index(shape_id) {
            if index == target {
                return;
            }
            let shape = self.shapes.remove(index);
            let mut insert_index = target.min(self.shapes.len());
            if index < insert_index && insert_index > 0 {
                insert_index -= 1;
            }
            self.shapes.insert(insert_index, shape);
        }
    }

    #[allow(dead_code)]
    pub fn primary_shape_for_undo(action: &UndoAction) -> Option<Shape> {
        match action {
            UndoAction::Create { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Delete { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Modify { before, .. } => Some(before.shape.clone()),
            UndoAction::Reorder { .. } => None,
            UndoAction::Compound(actions) => {
                actions.iter().rev().find_map(Self::primary_shape_for_undo)
            }
        }
    }

    #[allow(dead_code)]
    pub fn primary_shape_for_redo(action: &UndoAction) -> Option<Shape> {
        match action {
            UndoAction::Create { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Delete { shapes } => shapes.first().map(|(_, s)| s.shape.clone()),
            UndoAction::Modify { after, .. } => Some(after.shape.clone()),
            UndoAction::Reorder { .. } => None,
            UndoAction::Compound(actions) => actions.iter().find_map(Self::primary_shape_for_redo),
        }
    }

    fn generate_id(&mut self) -> ShapeId {
        let id = self.next_shape_id;
        self.next_shape_id = self.next_shape_id.saturating_add(1);
        id
    }

    fn mark_id_used(&mut self, id: ShapeId) {
        if id >= self.next_shape_id {
            self.next_shape_id = id.saturating_add(1);
        }
    }

    fn rebuild_next_id(&mut self) {
        self.next_shape_id = self
            .shapes
            .iter()
            .map(|shape| shape.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
    }
}

impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FrameHelper {
            #[serde(with = "frame_storage")]
            shapes: Vec<DrawnShape>,
        }

        let helper = FrameHelper::deserialize(deserializer)?;
        let mut frame = Frame {
            shapes: helper.shapes,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            next_shape_id: 1,
        };
        frame.rebuild_next_id();
        Ok(frame)
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_millis() as u64)
        .unwrap_or(0)
}

mod frame_storage {
    use super::{DrawnShape, ShapeId, current_timestamp_ms};
    use crate::draw::shape::Shape;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(shapes: &Vec<DrawnShape>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let helper: Vec<SerializedDrawnShape<'_>> = shapes
            .iter()
            .map(|shape| SerializedDrawnShape {
                id: shape.id,
                shape: &shape.shape,
                created_at: shape.created_at,
                locked: shape.locked,
            })
            .collect();
        helper.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<DrawnShape>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helpers: Vec<ShapeElement> = Vec::deserialize(deserializer)?;
        let mut shapes = Vec::with_capacity(helpers.len());
        let mut next_id: ShapeId = 1;

        for entry in helpers {
            match entry {
                ShapeElement::WithMeta(helper) => {
                    let mut id = helper.id.unwrap_or(0);
                    if id == 0 {
                        id = next_id;
                    }
                    let created_at = helper.created_at.unwrap_or_else(current_timestamp_ms);
                    let locked = helper.locked.unwrap_or(false);
                    shapes.push(DrawnShape::with_metadata(
                        id,
                        helper.shape,
                        created_at,
                        locked,
                    ));
                    next_id = next_id.max(id.saturating_add(1));
                }
                ShapeElement::Legacy(shape) => {
                    let id = next_id;
                    shapes.push(DrawnShape::with_metadata(
                        id,
                        shape,
                        current_timestamp_ms(),
                        false,
                    ));
                    next_id = next_id.saturating_add(1);
                }
            }
        }

        Ok(shapes)
    }

    #[derive(Serialize)]
    struct SerializedDrawnShape<'a> {
        id: ShapeId,
        shape: &'a Shape,
        created_at: u64,
        locked: bool,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ShapeElement {
        WithMeta(WithMeta),
        Legacy(Shape),
    }

    #[derive(Deserialize)]
    struct WithMeta {
        #[serde(default)]
        id: Option<ShapeId>,
        shape: Shape,
        #[serde(default)]
        created_at: Option<u64>,
        #[serde(default)]
        locked: Option<bool>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{Shape, color::BLACK};

    #[test]
    fn try_add_shape_respects_limit() {
        let mut frame = Frame::new();
        assert!(frame.try_add_shape(
            Shape::Line {
                x1: 0,
                y1: 0,
                x2: 1,
                y2: 1,
                color: BLACK,
                thick: 2.0,
            },
            1
        ));
        assert!(!frame.try_add_shape(
            Shape::Line {
                x1: 1,
                y1: 1,
                x2: 2,
                y2: 2,
                color: BLACK,
                thick: 2.0,
            },
            1
        ));
    }

    #[test]
    fn undo_and_redo_cycle_shapes() {
        let mut frame = Frame::new();
        let shape = Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: BLACK,
            thick: 2.0,
        };

        let id = frame.add_shape(shape.clone());
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(
                    frame.shapes.len().saturating_sub(1),
                    frame.shape(id).unwrap().clone(),
                )],
            },
            10,
        );
        assert_eq!(frame.shapes.len(), 1);

        let undone = frame.undo_last();
        assert!(undone.is_some());
        assert_eq!(frame.shapes.len(), 0);

        let redone = frame.redo_last();
        assert!(redone.is_some());
        assert_eq!(frame.shapes.len(), 1);
    }

    #[test]
    fn adding_new_shape_clears_redo_stack() {
        let mut frame = Frame::new();
        let first = Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            color: BLACK,
            thick: 2.0,
        };
        let id = frame.add_shape(first);
        frame.push_undo_action(
            UndoAction::Create {
                shapes: vec![(
                    frame.shapes.len().saturating_sub(1),
                    frame.shape(id).unwrap().clone(),
                )],
            },
            10,
        );
        frame.undo_last();
        assert_eq!(frame.shapes.len(), 0);

        let second = Shape::Rect {
            x: 10,
            y: 10,
            w: 15,
            h: 15,
            color: BLACK,
            thick: 2.0,
        };
        frame.add_shape(second);
        assert_eq!(frame.redo_stack_len(), 0);
    }

    #[test]
    fn undo_stack_respects_limit() {
        let mut frame = Frame::new();
        for i in 0..5 {
            let shape = Shape::Line {
                x1: i,
                y1: 0,
                x2: i + 10,
                y2: 10,
                color: BLACK,
                thick: 2.0,
            };
            let id = frame.add_shape(shape);
            let index = frame.find_index(id).unwrap();
            let snapshot = frame.shape(id).unwrap().clone();
            frame.push_undo_action(
                UndoAction::Create {
                    shapes: vec![(index, snapshot)],
                },
                3,
            );
        }

        assert_eq!(frame.undo_stack_len(), 3);
    }
}
