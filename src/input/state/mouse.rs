use crate::draw::{Shape, frame::UndoAction};
use crate::input::{events::MouseButton, tool::Tool};
use crate::util;
use log::warn;

use super::{ContextMenuKind, DrawingState, InputState};

impl InputState {
    fn handle_right_click(&mut self, x: i32, y: i32) {
        self.update_pointer_position(x, y);
        if !self.context_menu_enabled() {
            return;
        }
        if !matches!(self.state, DrawingState::Idle) {
            self.clear_provisional_dirty();
            self.last_provisional_bounds = None;
            self.state = DrawingState::Idle;
            self.needs_redraw = true;
            return;
        }

        let hit_shape = self.hit_test_at(x, y);
        if let Some(id) = hit_shape {
            if self.modifiers.shift {
                self.extend_selection([id]);
            } else if !self.selected_shape_ids().contains(&id) {
                self.set_selection(vec![id]);
            }
            let selection = self.selected_shape_ids().to_vec();
            self.open_context_menu((x, y), selection, ContextMenuKind::Shape);
        } else {
            self.clear_selection();
            self.open_context_menu((x, y), Vec::new(), ContextMenuKind::Canvas);
        }

        self.update_context_menu_hover_from_pointer(x, y);
        self.needs_redraw = true;
    }

    fn is_point_in_context_menu(&self, x: i32, y: i32) -> bool {
        if let Some(layout) = self.context_menu_layout() {
            let xf = x as f64;
            let yf = y as f64;
            xf >= layout.origin_x
                && xf <= layout.origin_x + layout.width
                && yf >= layout.origin_y
                && yf <= layout.origin_y + layout.height
        } else {
            false
        }
    }

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
        self.close_properties_panel();
        match button {
            MouseButton::Right => {
                self.handle_right_click(x, y);
            }
            MouseButton::Left => {
                self.update_pointer_position(x, y);
                self.trigger_click_highlight(x, y);

                if self.is_context_menu_open() {
                    if self.is_point_in_context_menu(x, y) {
                        self.update_context_menu_hover_from_pointer(x, y);
                    } else {
                        self.close_context_menu();
                        self.needs_redraw = true;
                    }
                    return;
                }

                if matches!(self.state, DrawingState::Idle) {
                    let tool = self.active_tool();
                    if tool != Tool::Highlight {
                        self.state = DrawingState::Drawing {
                            tool,
                            start_x: x,
                            start_y: y,
                            points: vec![(x, y)],
                        };
                        self.last_provisional_bounds = None;
                        self.update_provisional_dirty(x, y);
                        self.needs_redraw = true;
                    }
                } else if let DrawingState::TextInput { x: tx, y: ty, .. } = &mut self.state {
                    *tx = x;
                    *ty = y;
                    self.update_text_preview_dirty();
                    self.needs_redraw = true;
                }
            }
            MouseButton::Middle => {}
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
        self.update_pointer_position(x, y);
        if self.is_context_menu_open() {
            self.update_context_menu_hover_from_pointer(x, y);
            return;
        }

        let mut drawing = false;
        if let DrawingState::Drawing { tool, points, .. } = &mut self.state {
            if *tool == Tool::Pen {
                points.push((x, y));
            }
            drawing = true;
        }

        if drawing {
            self.update_provisional_dirty(x, y);
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
        self.update_pointer_position(x, y);
        if button == MouseButton::Left && self.is_context_menu_open() {
            if let Some(index) = self.context_menu_index_at(x, y) {
                let entries = self.context_menu_entries();
                if let Some(entry) = entries.get(index) {
                    if !entry.disabled {
                        if let Some(command) = entry.command {
                            self.execute_menu_command(command);
                        } else {
                            self.close_context_menu();
                        }
                    } else {
                        self.close_context_menu();
                    }
                }
            } else {
                self.close_context_menu();
            }
            self.needs_redraw = true;
            return;
        }

        if button != MouseButton::Left {
            return;
        }

        let state = std::mem::replace(&mut self.state, DrawingState::Idle);
        if let DrawingState::Drawing {
            tool,
            start_x,
            start_y,
            points,
        } = state
        {
            let shape = match tool {
                Tool::Pen => Shape::Freehand {
                    points,
                    color: self.current_color,
                    thick: self.current_thickness,
                },
                Tool::Line => Shape::Line {
                    x1: start_x,
                    y1: start_y,
                    x2: x,
                    y2: y,
                    color: self.current_color,
                    thick: self.current_thickness,
                },
                Tool::Rect => {
                    let (left, width) = if x >= start_x {
                        (start_x, x - start_x)
                    } else {
                        (x, start_x - x)
                    };
                    let (top, height) = if y >= start_y {
                        (start_y, y - start_y)
                    } else {
                        (y, start_y - y)
                    };
                    Shape::Rect {
                        x: left,
                        y: top,
                        w: width,
                        h: height,
                        color: self.current_color,
                        thick: self.current_thickness,
                    }
                }
                Tool::Ellipse => {
                    let (cx, cy, rx, ry) = util::ellipse_bounds(start_x, start_y, x, y);
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
                    x1: start_x,
                    y1: start_y,
                    x2: x,
                    y2: y,
                    color: self.current_color,
                    thick: self.current_thickness,
                    arrow_length: self.arrow_length,
                    arrow_angle: self.arrow_angle,
                },
                Tool::Highlight => {
                    self.clear_provisional_dirty();
                    return;
                }
            };

            let bounds = shape.bounding_box();
            self.clear_provisional_dirty();

            let mut limit_reached = false;
            let addition = {
                let frame = self.canvas_set.active_frame_mut();
                match frame.try_add_shape_with_id(shape.clone(), self.max_shapes_per_frame) {
                    Some(new_id) => {
                        if let Some(index) = frame.find_index(new_id) {
                            if let Some(new_shape) = frame.shape(new_id) {
                                let snapshot = new_shape.clone();
                                frame.push_undo_action(
                                    UndoAction::Create {
                                        shapes: vec![(index, snapshot.clone())],
                                    },
                                    self.undo_stack_limit,
                                );
                                Some((new_id, snapshot))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    None => {
                        limit_reached = true;
                        None
                    }
                }
            };

            if let Some((new_id, _snapshot)) = addition {
                self.invalidate_hit_cache_for(new_id);
                self.dirty_tracker.mark_optional_rect(bounds);
                self.clear_selection();
                self.needs_redraw = true;
            } else if limit_reached {
                warn!(
                    "Shape limit ({}) reached; discarding new shape",
                    self.max_shapes_per_frame
                );
            }
        } else {
            self.state = state;
        }
    }
}
