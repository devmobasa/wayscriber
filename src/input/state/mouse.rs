use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::Shape;
use crate::input::{EraserMode, Tool, events::MouseButton};
use crate::util;
use log::warn;
use std::time::Instant;

use super::{ContextMenuKind, DrawingState, InputState};
use super::core::TextClickState;
use super::core::MenuCommand;

const TEXT_CLICK_DRAG_THRESHOLD: i32 = 4;
const TEXT_DOUBLE_CLICK_MS: u64 = 400;
const TEXT_DOUBLE_CLICK_DISTANCE: i32 = 6;

impl InputState {
    fn handle_right_click(&mut self, x: i32, y: i32) {
        self.update_pointer_position(x, y);
        self.last_text_click = None;
        if !matches!(self.state, DrawingState::Idle) {
            match &self.state {
                DrawingState::TextInput { .. } => {
                    self.cancel_text_input();
                }
                DrawingState::MovingSelection { snapshots, .. } => {
                    self.restore_selection_from_snapshots(snapshots.clone());
                    self.state = DrawingState::Idle;
                }
                DrawingState::ResizingText {
                    shape_id,
                    snapshot,
                    ..
                } => {
                    self.restore_selection_from_snapshots(vec![(*shape_id, snapshot.clone())]);
                    self.state = DrawingState::Idle;
                }
                DrawingState::PendingTextClick { .. } => {
                    self.state = DrawingState::Idle;
                }
                _ => {
                    self.clear_provisional_dirty();
                    self.last_provisional_bounds = None;
                    self.state = DrawingState::Idle;
                    self.needs_redraw = true;
                }
            }
            return;
        }
        if self.zoom_active() {
            return;
        }
        if !self.context_menu_enabled() {
            return;
        }

        let hit_shape = self.hit_test_at(x, y);
        let mut focus_edit = false;
        if let Some(id) = hit_shape {
            if self.modifiers.shift {
                self.extend_selection([id]);
            } else if !self.selected_shape_ids().contains(&id) {
                self.set_selection(vec![id]);
            }
            let selection = self.selected_shape_ids().to_vec();
            focus_edit = selection.len() == 1
                && self
                    .canvas_set
                    .active_frame()
                    .shape(selection[0])
                    .map(|shape| {
                        matches!(shape.shape, Shape::Text { .. } | Shape::StickyNote { .. })
                    })
                    .unwrap_or(false);
            self.open_context_menu((x, y), selection, ContextMenuKind::Shape, hit_shape);
        } else {
            self.clear_selection();
            self.open_context_menu((x, y), Vec::new(), ContextMenuKind::Canvas, None);
        }

        self.update_context_menu_hover_from_pointer(x, y);
        if focus_edit {
            self.focus_context_menu_command(MenuCommand::EditText);
        }
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
                    self.last_text_click = None;
                    if self.is_point_in_context_menu(x, y) {
                        self.update_context_menu_hover_from_pointer(x, y);
                    } else {
                        self.close_context_menu();
                        self.needs_redraw = true;
                    }
                    return;
                }

                match &mut self.state {
                    DrawingState::Idle => {
                        let selection_click = self.modifiers.alt;
                        if let Some(shape_id) = self.hit_text_resize_handle(x, y) {
                            let snapshot = {
                                let frame = self.canvas_set.active_frame();
                                frame.shape(shape_id).map(|shape| ShapeSnapshot {
                                    shape: shape.shape.clone(),
                                    locked: shape.locked,
                                })
                            };
                            if let Some(snapshot) = snapshot {
                                let (base_x, size) = match &snapshot.shape {
                                    Shape::Text { x, size, .. } => (*x, *size),
                                    Shape::StickyNote { x, size, .. } => (*x, *size),
                                    _ => return,
                                };
                                self.last_text_click = None;
                                self.state = DrawingState::ResizingText {
                                    shape_id,
                                    snapshot,
                                    base_x,
                                    size,
                                };
                                return;
                            }
                        }

                        if !selection_click {
                            if let Some(hit_id) = self.hit_test_at(x, y) {
                                let is_text = self
                                    .canvas_set
                                    .active_frame()
                                    .shape(hit_id)
                                    .map(|shape| {
                                        !shape.locked
                                            && matches!(
                                                shape.shape,
                                                Shape::Text { .. } | Shape::StickyNote { .. }
                                            )
                                    })
                                    .unwrap_or(false);
                                if is_text {
                                    self.state = DrawingState::PendingTextClick {
                                        x,
                                        y,
                                        tool: self.active_tool(),
                                        shape_id: hit_id,
                                    };
                                    return;
                                }
                            }
                        }
                        self.last_text_click = None;
                        if selection_click && let Some(hit_id) = self.hit_test_at(x, y) {
                            if !self.selected_shape_ids().contains(&hit_id) {
                                if self.modifiers.shift {
                                    self.extend_selection([hit_id]);
                                } else {
                                    self.set_selection(vec![hit_id]);
                                }
                            }

                            let snapshots = self.capture_movable_selection_snapshots();
                            if !snapshots.is_empty() {
                                self.state = DrawingState::MovingSelection {
                                    last_x: x,
                                    last_y: y,
                                    snapshots,
                                    moved: false,
                                };
                                return;
                            }
                        }

                        let tool = self.active_tool();
                        if tool != Tool::Highlight && tool != Tool::Select {
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
                    }
                    DrawingState::TextInput { x: tx, y: ty, .. } => {
                        *tx = x;
                        *ty = y;
                        self.update_text_preview_dirty();
                        self.needs_redraw = true;
                    }
                    DrawingState::Drawing { .. }
                    | DrawingState::MovingSelection { .. }
                    | DrawingState::PendingTextClick { .. }
                    | DrawingState::ResizingText { .. } => {}
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

        if let DrawingState::ResizingText {
            shape_id,
            base_x,
            size,
            ..
        } = &self.state
        {
            let new_width = self.clamp_text_wrap_width(*base_x, x, *size);
            let _ = self.update_text_wrap_width(*shape_id, new_width);
            return;
        }

        if let DrawingState::PendingTextClick {
            x: start_x,
            y: start_y,
            tool,
            ..
        } = &self.state
        {
            let dx = x - *start_x;
            let dy = y - *start_y;
            if dx.abs() >= TEXT_CLICK_DRAG_THRESHOLD || dy.abs() >= TEXT_CLICK_DRAG_THRESHOLD {
                let tool = *tool;
                if tool != Tool::Highlight && tool != Tool::Select {
                    let mut points = vec![(*start_x, *start_y)];
                    if tool == Tool::Pen || tool == Tool::Marker || tool == Tool::Eraser {
                        points.push((x, y));
                    }
                    self.state = DrawingState::Drawing {
                        tool,
                        start_x: *start_x,
                        start_y: *start_y,
                        points,
                    };
                    self.last_text_click = None;
                    self.last_provisional_bounds = None;
                    self.update_provisional_dirty(x, y);
                    self.needs_redraw = true;
                }
            }
            return;
        }

        if let DrawingState::MovingSelection { last_x, last_y, .. } = &self.state {
            let dx = x - *last_x;
            let dy = y - *last_y;
            if (dx != 0 || dy != 0)
                && self.apply_translation_to_selection(dx, dy)
                && let DrawingState::MovingSelection {
                    last_x,
                    last_y,
                    moved,
                    ..
                } = &mut self.state
            {
                *last_x = x;
                *last_y = y;
                *moved = true;
            }
            return;
        }

        if self.is_context_menu_open() {
            self.update_context_menu_hover_from_pointer(x, y);
            return;
        }

        let mut drawing = false;
        if let DrawingState::Drawing { tool, points, .. } = &mut self.state {
            if *tool == Tool::Pen || *tool == Tool::Marker || *tool == Tool::Eraser {
                points.push((x, y));
            }
            drawing = true;
        }

        if drawing {
            self.update_provisional_dirty(x, y);
            self.needs_redraw = true;
        } else if self.eraser_mode == EraserMode::Stroke
            && self.active_tool() == Tool::Eraser
            && matches!(self.state, DrawingState::Idle)
        {
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
        match state {
            DrawingState::MovingSelection {
                snapshots, moved, ..
            } => {
                if moved {
                    self.push_translation_undo(snapshots);
                }
            }
            DrawingState::ResizingText {
                shape_id,
                snapshot,
                ..
            } => {
                let frame = self.canvas_set.active_frame_mut();
                if let Some(shape) = frame.shape(shape_id) {
                    let after_snapshot = ShapeSnapshot {
                        shape: shape.shape.clone(),
                        locked: shape.locked,
                    };
                    let before_wrap = match &snapshot.shape {
                        Shape::Text { wrap_width, .. }
                        | Shape::StickyNote { wrap_width, .. } => *wrap_width,
                        _ => None,
                    };
                    let after_wrap = match &after_snapshot.shape {
                        Shape::Text { wrap_width, .. }
                        | Shape::StickyNote { wrap_width, .. } => *wrap_width,
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
