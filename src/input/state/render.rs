use crate::draw::{
    Color, Shape, render_freehand_borrowed, render_marker_stroke_borrowed, render_shape,
};
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
                        fill: self.fill_enabled,
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
                        fill: self.fill_enabled,
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
                    head_at_end: self.arrow_head_at_end,
                    label: self.next_arrow_label(),
                }),
                Tool::Marker => Some(Shape::MarkerStroke {
                    points: points.clone(),
                    color: self.marker_color(),
                    thick: self.current_thickness,
                }),
                Tool::Eraser => None, // Preview handled separately to avoid clearing the buffer
                Tool::Highlight => None,
                Tool::Select => None,
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
        match &self.state {
            DrawingState::Drawing {
                tool,
                start_x: _,
                start_y: _,
                points,
            } => match tool {
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
                Tool::Marker => {
                    render_marker_stroke_borrowed(
                        ctx,
                        points,
                        self.marker_color(),
                        self.current_thickness,
                    );
                    true
                }
                Tool::Eraser => {
                    // Visual preview without actually clearing
                    let preview_color = Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 0.35,
                    };
                    render_freehand_borrowed(ctx, points, preview_color, self.eraser_size);
                    true
                }
                _ => {
                    // For other tools, use the normal path (no clone needed)
                    if let Some(shape) = self.get_provisional_shape(current_x, current_y) {
                        render_shape(ctx, &shape);
                        true
                    } else {
                        false
                    }
                }
            },
            DrawingState::Selecting {
                start_x,
                start_y,
                additive,
            } => {
                let Some(rect) =
                    Self::selection_rect_from_points(*start_x, *start_y, current_x, current_y)
                else {
                    return false;
                };
                let _ = ctx.save();
                ctx.rectangle(
                    rect.x as f64,
                    rect.y as f64,
                    rect.width as f64,
                    rect.height as f64,
                );
                ctx.set_source_rgba(0.2, 0.45, 1.0, 0.12);
                let _ = ctx.fill_preserve();
                if *additive {
                    ctx.set_source_rgba(0.2, 0.75, 0.45, 0.9);
                } else {
                    ctx.set_source_rgba(0.2, 0.45, 1.0, 0.9);
                }
                ctx.set_line_width(1.5);
                ctx.set_dash(&[6.0, 4.0], 0.0);
                let _ = ctx.stroke();
                let _ = ctx.restore();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn marker_color(&self) -> Color {
        // Keep a minimum alpha so the marker remains visible even if a fully transparent color was set.
        let alpha = (self.current_color.a * self.marker_opacity).clamp(0.05, 0.9);
        Color {
            a: alpha,
            ..self.current_color
        }
    }
}
