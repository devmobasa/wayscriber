//! Frame container for managing collections of shapes.

use super::shape::Shape;
use serde::{Deserialize, Serialize};

/// Container for all shapes in the current drawing session.
///
/// Manages a collection of [`Shape`]s and provides operations like adding,
/// clearing, and undoing shapes. Acts as the drawing canvas state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Vector of all shapes in draw order (first = bottom layer, last = top layer)
    pub shapes: Vec<Shape>,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    /// Creates a new empty frame with no shapes.
    pub fn new() -> Self {
        Self { shapes: Vec::new() }
    }

    /// Removes all shapes from the frame, clearing the canvas.
    pub fn clear(&mut self) {
        self.shapes.clear();
    }

    /// Adds a new shape to the frame (drawn on top of existing shapes).
    pub fn add_shape(&mut self, shape: Shape) {
        self.shapes.push(shape);
    }

    /// Removes the most recently added shape.
    ///
    /// Returns `true` if a shape was removed, `false` if the frame was already empty.
    pub fn undo(&mut self) -> bool {
        if self.shapes.is_empty() {
            false
        } else {
            self.shapes.pop();
            true
        }
    }
}
