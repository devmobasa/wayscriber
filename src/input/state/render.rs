use crate::draw::{Shape, render_freehand_borrowed, render_shape};
use crate::input::tool::Tool;
use crate::util;

use super::{DrawingState, InputState};

impl InputState {
    /// Returns the shape currently being drawn for live preview.
    ///
    /// # Arguments
    /// * `current_x` - Current mouse X coordinate
    /// * `current_y` - Current mouse Y coordinate
    ///
    /// # Returns
    /// - `Some(Shape)` if actively drawing (for preview rendering)
    /// - `None` if idle or in text input mode
    ///
    /// # Note
    /// For Pen tool (freehand), this clones the points vector. For better performance
    /// with long strokes, consider using `render_provisional_shape` directly with a
    /// borrow instead of calling this method and rendering separately.
    ///
    /// This allows the backend to render a preview of the shape being drawn
    /// before the mouse button is released.
    pub fn get_provisional_shape(&self, current_x: i32, current_y: i32) -> Option<Shape> {
        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = &self.state
        {
            match tool {
                Tool::Pen => Some(Shape::Freehand {
                    points: points.clone(), // TODO: Consider using Cow or separate borrow API
                    color: self.current_color,
                    thick: self.current_thickness,
                }),
                Tool::Line => Some(Shape::Line {
                    x1: *start_x,
                    y1: *start_y,
                    x2: current_x,
                    y2: current_y,
                    color: self.current_color,
                    thick: self.current_thickness,
                }),
                Tool::Rect => {
                    // Normalize rectangle to handle dragging in any direction
                    let (x, w) = if current_x >= *start_x {
                        (*start_x, current_x - start_x)
                    } else {
                        (current_x, start_x - current_x)
                    };
                    let (y, h) = if current_y >= *start_y {
                        (*start_y, current_y - start_y)
                    } else {
                        (current_y, start_y - current_y)
                    };
                    Some(Shape::Rect {
                        x,
                        y,
                        w,
                        h,
                        color: self.current_color,
                        thick: self.current_thickness,
                    })
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) =
                        util::ellipse_bounds(*start_x, *start_y, current_x, current_y);
                    Some(Shape::Ellipse {
                        cx,
                        cy,
                        rx,
                        ry,
                        color: self.current_color,
                        thick: self.current_thickness,
                    })
                }
                Tool::Arrow => Some(Shape::Arrow {
                    x1: *start_x,
                    y1: *start_y,
                    x2: current_x,
                    y2: current_y,
                    color: self.current_color,
                    thick: self.current_thickness,
                    arrow_length: self.arrow_length,
                    arrow_angle: self.arrow_angle,
                }),
                Tool::Highlight => None,
                // No provisional shape for other tools
            }
        } else {
            None
        }
    }

    /// Renders the provisional shape directly to a Cairo context without cloning.
    ///
    /// This is an optimized version for freehand drawing that avoids cloning
    /// the points vector on every render, preventing quadratic performance.
    ///
    /// # Arguments
    /// * `ctx` - Cairo context to render to
    /// * `current_x` - Current mouse X coordinate
    /// * `current_y` - Current mouse Y coordinate
    ///
    /// # Returns
    /// `true` if a provisional shape was rendered, `false` otherwise
    pub fn render_provisional_shape(
        &self,
        ctx: &cairo::Context,
        current_x: i32,
        current_y: i32,
    ) -> bool {
        if let DrawingState::Drawing {
            tool,
            start_x: _,
            start_y: _,
            points,
        } = &self.state
        {
            match tool {
                Tool::Pen => {
                    // Render freehand without cloning - just borrow the points
                    render_freehand_borrowed(
                        ctx,
                        points,
                        self.current_color,
                        self.current_thickness,
                    );
                    true
                }
                Tool::Highlight => false,
                _ => {
                    // For other tools, use the normal path (no clone needed)
                    if let Some(shape) = self.get_provisional_shape(current_x, current_y) {
                        render_shape(ctx, &shape);
                        true
                    } else {
                        false
                    }
                }
            }
        } else {
            false
        }
    }
}
