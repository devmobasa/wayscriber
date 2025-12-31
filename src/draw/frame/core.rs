use super::frame_storage;
use super::types::{DrawnShape, ShapeId, UndoAction};
use crate::draw::shape::Shape;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Frame {
    #[serde(with = "frame_storage")]
    pub shapes: Vec<DrawnShape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) undo_stack: Vec<UndoAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) redo_stack: Vec<UndoAction>,
    #[serde(skip)]
    pub(super) next_shape_id: ShapeId,
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
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.next_shape_id = 1;
    }

    /// Clone the frame's shapes while dropping history.
    pub fn clone_without_history(&self) -> Self {
        let mut frame = Frame::new();
        frame.shapes = self.shapes.clone();
        frame.rebuild_next_id();
        frame
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

    /// Returns true if shapes or history stacks contain data worth persisting.
    pub fn has_persistable_data(&self) -> bool {
        !self.shapes.is_empty() || !self.undo_stack.is_empty() || !self.redo_stack.is_empty()
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

    pub(super) fn insert_new_shape(&mut self, index: usize, shape: Shape) -> ShapeId {
        let id = self.generate_id();
        let drawn = DrawnShape::new(id, shape);
        self.insert_existing(index, drawn);
        id
    }

    pub(super) fn insert_existing(&mut self, index: usize, drawn: DrawnShape) {
        self.mark_id_used(drawn.id);
        self.shapes.insert(index, drawn);
    }

    pub(super) fn generate_id(&mut self) -> ShapeId {
        let id = self.next_shape_id;
        self.next_shape_id = self.next_shape_id.saturating_add(1);
        id
    }

    pub(super) fn mark_id_used(&mut self, id: ShapeId) {
        if id >= self.next_shape_id {
            self.next_shape_id = id.saturating_add(1);
        }
    }

    pub(super) fn rebuild_next_id(&mut self) {
        let shapes_max = self.shapes.iter().map(|shape| shape.id).max().unwrap_or(0);
        let history_max = self
            .undo_stack
            .iter()
            .chain(self.redo_stack.iter())
            .filter_map(|action| action.max_shape_id())
            .max()
            .unwrap_or(0);

        self.next_shape_id = shapes_max.max(history_max).saturating_add(1);
    }
}
