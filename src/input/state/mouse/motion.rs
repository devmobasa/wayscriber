use crate::input::{EraserMode, Tool};

use super::super::{DrawingState, InputState};
use super::TEXT_CLICK_DRAG_THRESHOLD;
use std::sync::Arc;

impl InputState {
    /// Processes mouse motion (dragging) events.
    ///
    /// # Arguments
    /// * `x` - Current mouse X coordinate
    /// * `y` - Mouse Y coordinate
    ///
    /// # Behavior
    /// - When drawing with Pen tool: Adds points to the freehand stroke
    /// - When drawing with other tools: Triggers redraw for live preview
    pub fn on_mouse_motion(&mut self, x: i32, y: i32) {
        self.update_pointer_position(x, y);

        if self.is_color_picker_popup_open() {
            if self.color_picker_popup_is_dragging()
                && let Some(layout) = self.color_picker_popup_layout()
            {
                let fx = x as f64;
                let fy = y as f64;
                let norm_x = ((fx - layout.gradient_x) / layout.gradient_w).clamp(0.0, 1.0);
                let norm_y = ((fy - layout.gradient_y) / layout.gradient_h).clamp(0.0, 1.0);
                self.color_picker_popup_set_from_gradient(norm_x, norm_y);
            }
            return;
        }

        if self.is_board_picker_open() {
            if self.board_picker_is_page_dragging() {
                self.board_picker_update_page_drag_from_pointer(x, y);
            } else if self.board_picker_is_dragging() {
                self.board_picker_update_drag_from_pointer(x, y);
            } else {
                self.update_board_picker_hover_from_pointer(x, y);
            }
            return;
        }

        if self.is_properties_panel_open() {
            if self.properties_panel_layout().is_none() {
                return;
            }
            self.update_properties_panel_hover_from_pointer(x, y);
            return;
        }

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
                    let mut point_thicknesses = vec![self.current_thickness as f32];
                    if tool == Tool::Pen || tool == Tool::Marker || tool == Tool::Eraser {
                        points.push((x, y));
                        point_thicknesses.push(self.current_thickness as f32);
                    }
                    self.state = DrawingState::Drawing {
                        tool,
                        start_x: *start_x,
                        start_y: *start_y,
                        points,
                        point_thicknesses,
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

        if let DrawingState::ResizingSelection {
            handle,
            original_bounds,
            start_x,
            start_y,
            snapshots,
        } = &self.state
        {
            let dx = x - *start_x;
            let dy = y - *start_y;
            let handle = *handle;
            let original_bounds = *original_bounds;
            let snapshots = Arc::clone(snapshots);
            self.apply_selection_resize(handle, &original_bounds, dx, dy, snapshots.as_ref());
            self.needs_redraw = true;
            return;
        }

        if matches!(self.state, DrawingState::Selecting { .. }) {
            self.update_provisional_dirty(x, y);
            self.needs_redraw = true;
            return;
        }

        if self.is_context_menu_open() {
            self.update_context_menu_hover_from_pointer(x, y);
            return;
        }

        let mut drawing = false;
        if let DrawingState::Drawing {
            tool,
            points,
            point_thicknesses,
            ..
        } = &mut self.state
        {
            if *tool == Tool::Pen || *tool == Tool::Marker || *tool == Tool::Eraser {
                points.push((x, y));
                point_thicknesses.push(self.current_thickness as f32);
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
}
