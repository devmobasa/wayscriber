use crate::draw::Shape;
use crate::draw::frame::ShapeSnapshot;
use crate::input::{Tool, events::MouseButton};

use super::super::core::MenuCommand;
use super::super::{ContextMenuKind, DrawingState, InputState};

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
                DrawingState::Selecting { .. } => {
                    self.clear_provisional_dirty();
                    self.last_provisional_bounds = None;
                    self.state = DrawingState::Idle;
                    self.needs_redraw = true;
                }
                DrawingState::ResizingText {
                    shape_id, snapshot, ..
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
                    .boards
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
        if self.is_board_picker_open() {
            self.update_pointer_position(x, y);
            match button {
                MouseButton::Left => {
                    if self.board_picker_contains_point(x, y) {
                        if let Some(row) = self.board_picker_handle_index_at(x, y) {
                            self.board_picker_start_drag(row);
                            return;
                        }
                        if self.board_picker_index_at(x, y).is_some() {
                            self.update_board_picker_hover_from_pointer(x, y);
                        }
                    } else {
                        self.close_board_picker();
                    }
                }
                MouseButton::Right => {
                    self.close_board_picker();
                }
                MouseButton::Middle => {}
            }
            return;
        }

        if self.is_properties_panel_open() {
            self.update_pointer_position(x, y);
            if self.properties_panel_layout().is_none() {
                return;
            }
            match button {
                MouseButton::Left => {
                    if let Some(index) = self.properties_panel_index_at(x, y) {
                        self.set_properties_panel_focus(Some(index));
                    } else {
                        self.close_properties_panel();
                    }
                }
                MouseButton::Right => {
                    self.close_properties_panel();
                }
                MouseButton::Middle => {}
            }
            return;
        }

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
                        let selection_click =
                            self.modifiers.alt || self.active_tool() == Tool::Select;
                        if let Some(shape_id) = self.hit_text_resize_handle(x, y) {
                            let snapshot = {
                                let frame = self.boards.active_frame();
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

                        if !selection_click && let Some(hit_id) = self.hit_test_at(x, y) {
                            let is_text = self
                                .boards
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
                        self.last_text_click = None;
                        if selection_click {
                            if let Some(hit_id) = self.hit_test_at(x, y) {
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
                            } else {
                                self.state = DrawingState::Selecting {
                                    start_x: x,
                                    start_y: y,
                                    additive: self.modifiers.shift,
                                };
                                self.last_provisional_bounds = None;
                                self.update_provisional_dirty(x, y);
                                self.needs_redraw = true;
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
                                point_thicknesses: vec![self.current_thickness as f32],
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
                    | DrawingState::Selecting { .. }
                    | DrawingState::PendingTextClick { .. }
                    | DrawingState::ResizingText { .. } => {}
                }
            }
            MouseButton::Middle => {}
        }
    }
}
