use crate::draw::Shape;
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::input::{EraserMode, Tool, events::MouseButton};
use crate::util;
use log::warn;
use std::time::Instant;

use super::super::core::TextClickState;
use super::super::{DrawingState, InputState};
use super::{SELECTION_DRAG_THRESHOLD, TEXT_DOUBLE_CLICK_DISTANCE, TEXT_DOUBLE_CLICK_MS};

impl InputState {
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
        if button == MouseButton::Left && self.is_properties_panel_open() {
            if self.properties_panel_layout().is_none() {
                return;
            }
            if let Some(index) = self.properties_panel_index_at(x, y) {
                self.set_properties_panel_focus(Some(index));
                self.activate_properties_panel_entry();
            } else {
                self.close_properties_panel();
            }
            self.needs_redraw = true;
            return;
        }

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
        match state {
            DrawingState::MovingSelection {
                snapshots, moved, ..
            } => {
                if moved {
                    self.push_translation_undo(snapshots);
                }
            }
            DrawingState::Selecting {
                start_x,
                start_y,
                additive,
            } => {
                self.clear_provisional_dirty();
                let dx = (x - start_x).abs();
                let dy = (y - start_y).abs();
                if dx < SELECTION_DRAG_THRESHOLD && dy < SELECTION_DRAG_THRESHOLD {
                    if !additive {
                        let bounds = self.selection_bounding_box(self.selected_shape_ids());
                        self.clear_selection();
                        self.mark_selection_dirty_region(bounds);
                        self.needs_redraw = true;
                    }
                    return;
                }

                if let Some(rect) = Self::selection_rect_from_points(start_x, start_y, x, y) {
                    let ids = self.shape_ids_in_rect(rect);
                    if additive {
                        self.extend_selection(ids);
                    } else {
                        self.set_selection(ids);
                    }
                    self.needs_redraw = true;
                }
            }
            DrawingState::ResizingText {
                shape_id, snapshot, ..
            } => {
                let frame = self.canvas_set.active_frame_mut();
                if let Some(shape) = frame.shape(shape_id) {
                    let after_snapshot = ShapeSnapshot {
                        shape: shape.shape.clone(),
                        locked: shape.locked,
                    };
                    let before_wrap = match &snapshot.shape {
                        Shape::Text { wrap_width, .. } | Shape::StickyNote { wrap_width, .. } => {
                            *wrap_width
                        }
                        _ => None,
                    };
                    let after_wrap = match &after_snapshot.shape {
                        Shape::Text { wrap_width, .. } | Shape::StickyNote { wrap_width, .. } => {
                            *wrap_width
                        }
                        _ => None,
                    };
                    if before_wrap != after_wrap {
                        frame.push_undo_action(
                            UndoAction::Modify {
                                shape_id,
                                before: snapshot,
                                after: after_snapshot,
                            },
                            self.undo_stack_limit,
                        );
                    }
                }
            }
            DrawingState::Drawing {
                tool,
                start_x,
                start_y,
                points,
            } => {
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
                            fill: self.fill_enabled,
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
                            fill: self.fill_enabled,
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
                        head_at_end: self.arrow_head_at_end,
                    },
                    Tool::Marker => Shape::MarkerStroke {
                        points,
                        color: self.marker_color(),
                        thick: self.current_thickness,
                    },
                    Tool::Eraser => {
                        if self.eraser_mode == EraserMode::Stroke {
                            self.clear_provisional_dirty();
                            let mut path = points;
                            if path.last().copied() != Some((x, y)) {
                                path.push((x, y));
                            }
                            self.erase_strokes_by_points(&path);
                            return;
                        }
                        Shape::EraserStroke {
                            points,
                            brush: crate::draw::shape::EraserBrush {
                                size: self.eraser_size,
                                kind: self.eraser_kind,
                            },
                        }
                    }
                    Tool::Highlight => {
                        self.clear_provisional_dirty();
                        return;
                    }
                    Tool::Select => {
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
            }
            DrawingState::PendingTextClick { x, y, shape_id, .. } => {
                let now = Instant::now();
                let is_double = self
                    .last_text_click
                    .map(|last| {
                        last.shape_id == shape_id
                            && now.duration_since(last.at).as_millis()
                                <= TEXT_DOUBLE_CLICK_MS as u128
                            && (x - last.x).abs() <= TEXT_DOUBLE_CLICK_DISTANCE
                            && (y - last.y).abs() <= TEXT_DOUBLE_CLICK_DISTANCE
                    })
                    .unwrap_or(false);

                if is_double {
                    self.last_text_click = None;
                    self.set_selection(vec![shape_id]);
                    let _ = self.edit_selected_text();
                } else {
                    self.last_text_click = Some(TextClickState {
                        shape_id,
                        x,
                        y,
                        at: now,
                    });
                }
            }
            other_state => {
                self.state = other_state;
            }
        }
    }
}
