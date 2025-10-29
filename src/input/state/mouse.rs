use crate::draw::Shape;
use crate::input::{events::MouseButton, tool::Tool};
use crate::util;
use log::warn;

use super::{DrawingState, InputState};

impl InputState {
    /// Processes a mouse button press event.
    ///
    /// # Arguments
    /// * `button` - Which mouse button was pressed
    /// * `x` - Mouse X coordinate
    /// * `y` - Mouse Y coordinate
    ///
    /// # Behavior
    /// - Left click while Idle: Starts drawing with the current tool (based on modifiers)
    /// - Left click during TextInput: Updates text position
    /// - Right click: Cancels current action
    pub fn on_mouse_press(&mut self, button: MouseButton, x: i32, y: i32) {
        match button {
            MouseButton::Left => {
                // Start drawing with current tool
                if matches!(self.state, DrawingState::Idle) {
                    let tool = self.modifiers.current_tool();
                    self.state = DrawingState::Drawing {
                        tool,
                        start_x: x,
                        start_y: y,
                        points: vec![(x, y)],
                    };
                    self.needs_redraw = true;
                } else if let DrawingState::TextInput { x: tx, y: ty, .. } = &mut self.state {
                    // Update text position if in text mode
                    *tx = x;
                    *ty = y;
                    self.needs_redraw = true;
                }
            }
            MouseButton::Right => {
                // Right-click could cancel or exit
                if !matches!(self.state, DrawingState::Idle) {
                    self.state = DrawingState::Idle;
                    self.needs_redraw = true;
                }
            }
            _ => {}
        }
    }

    /// Processes mouse motion (dragging) events.
    ///
    /// # Arguments
    /// * `x` - Current mouse X coordinate
    /// * `y` - Current mouse Y coordinate
    ///
    /// # Behavior
    /// - When drawing with Pen tool: Adds points to the freehand stroke
    /// - When drawing with other tools: Triggers redraw for live preview
    pub fn on_mouse_motion(&mut self, x: i32, y: i32) {
        if let DrawingState::Drawing { tool, points, .. } = &mut self.state {
            if *tool == Tool::Pen {
                // Add point to freehand stroke
                points.push((x, y));
            }
            // For other tools, we'll update the end point in release
            self.needs_redraw = true;
        }
    }

    /// Processes mouse button release events.
    ///
    /// # Arguments
    /// * `button` - Which mouse button was released
    /// * `x` - Mouse X coordinate at release
    /// * `y` - Mouse Y coordinate at release
    ///
    /// # Behavior
    /// When left button is released during drawing:
    /// - Finalizes the shape using start position and current position
    /// - Adds the completed shape to the frame
    /// - Returns to Idle state
    pub fn on_mouse_release(&mut self, button: MouseButton, x: i32, y: i32) {
        if button != MouseButton::Left {
            return;
        }

        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = &self.state
        {
            let shape = match tool {
                Tool::Pen => Shape::Freehand {
                    points: points.clone(),
                    color: self.current_color,
                    thick: self.current_thickness,
                },
                Tool::Line => Shape::Line {
                    x1: *start_x,
                    y1: *start_y,
                    x2: x,
                    y2: y,
                    color: self.current_color,
                    thick: self.current_thickness,
                },
                Tool::Rect => {
                    // Normalize rectangle to handle dragging in any direction
                    let (x, w) = if x >= *start_x {
                        (*start_x, x - start_x)
                    } else {
                        (x, start_x - x)
                    };
                    let (y, h) = if y >= *start_y {
                        (*start_y, y - start_y)
                    } else {
                        (y, start_y - y)
                    };
                    Shape::Rect {
                        x,
                        y,
                        w,
                        h,
                        color: self.current_color,
                        thick: self.current_thickness,
                    }
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) = util::ellipse_bounds(*start_x, *start_y, x, y);
                    Shape::Ellipse {
                        cx,
                        cy,
                        rx,
                        ry,
                        color: self.current_color,
                        thick: self.current_thickness,
                    }
                }
                Tool::Arrow => Shape::Arrow {
                    x1: *start_x,
                    y1: *start_y,
                    x2: x,
                    y2: y,
                    color: self.current_color,
                    thick: self.current_thickness,
                    arrow_length: self.arrow_length,
                    arrow_angle: self.arrow_angle,
                },
            };

            if self
                .canvas_set
                .active_frame_mut()
                .try_add_shape(shape, self.max_shapes_per_frame)
            {
                self.state = DrawingState::Idle;
                self.needs_redraw = true;
            } else {
                warn!(
                    "Shape limit ({}) reached; discarding new shape",
                    self.max_shapes_per_frame
                );
                self.state = DrawingState::Idle;
            }
        }
    }
}
