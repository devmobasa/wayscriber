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
    /// Shapes removed via undo that can be reapplied with redo
    #[serde(skip, default)]
    redo_stack: Vec<Shape>,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    /// Creates a new empty frame with no shapes.
    pub const fn new() -> Self {
        Self {
            shapes: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Removes all shapes from the frame, clearing the canvas.
    pub fn clear(&mut self) {
        self.shapes.clear();
        self.redo_stack.clear();
    }

    /// Adds a new shape to the frame (drawn on top of existing shapes).
    #[allow(dead_code)]
    pub fn add_shape(&mut self, shape: Shape) {
        self.redo_stack.clear();
        self.shapes.push(shape);
    }

    /// Attempts to add a shape, enforcing a maximum shape count when `max` > 0.
    ///
    /// Returns `true` if the shape was added, `false` if the limit would be exceeded.
    pub fn try_add_shape(&mut self, shape: Shape, max: usize) -> bool {
        if max == 0 || self.shapes.len() < max {
            self.redo_stack.clear();
            self.shapes.push(shape);
            true
        } else {
            false
        }
    }

    /// Removes and returns the most recently added shape, if any.
    pub fn undo(&mut self) -> Option<Shape> {
        if let Some(shape) = self.shapes.pop() {
            self.redo_stack.push(shape.clone());
            Some(shape)
        } else {
            None
        }
    }

    /// Re-applies the most recently undone shape, if any.
    pub fn redo(&mut self) -> Option<Shape> {
        if let Some(shape) = self.redo_stack.pop() {
            self.shapes.push(shape.clone());
            Some(shape)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Color;

    #[test]
    fn try_add_shape_respects_limit() {
        let mut frame = Frame::new();
        assert!(frame.try_add_shape(
            Shape::Line {
                x1: 0,
                y1: 0,
                x2: 1,
                y2: 1,
                color: Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
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
                color: Color {
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                },
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
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        };

        frame.add_shape(shape.clone());
        assert_eq!(frame.shapes.len(), 1);

        let undone = frame.undo();
        assert!(undone.is_some());
        assert_eq!(frame.shapes.len(), 0);

        let redone = frame.redo();
        assert!(redone.is_some());
        assert_eq!(frame.shapes.len(), 1);
    }

    #[test]
    fn adding_new_shape_clears_redo_stack() {
        let mut frame = Frame::new();
        let shape1 = Shape::Line {
            x1: 0,
            y1: 0,
            x2: 10,
            y2: 10,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        };
        let shape2 = Shape::Line {
            x1: 5,
            y1: 5,
            x2: 15,
            y2: 15,
            color: Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            thick: 2.0,
        };

        frame.add_shape(shape1);
        frame.undo();
        assert!(frame.redo().is_some());
        frame.undo();
        assert_eq!(frame.shapes.len(), 0);

        frame.add_shape(shape2);
        assert!(
            frame.redo().is_none(),
            "redo stack should clear on new shape"
        );
    }
}
