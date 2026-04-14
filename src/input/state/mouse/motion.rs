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
    #[allow(dead_code)] // Retained for older callers that only have canvas coordinates.
    pub fn on_mouse_motion(&mut self, x: i32, y: i32) {
        let (screen_x, screen_y) = self.screen_coords_for_canvas(x, y);
        self.on_mouse_motion_with_canvas(screen_x, screen_y, x, y);
    }

    pub fn on_mouse_motion_with_canvas(
        &mut self,
        screen_x: i32,
        screen_y: i32,
        canvas_x: i32,
        canvas_y: i32,
    ) {
        self.update_pointer_positions(screen_x, screen_y, canvas_x, canvas_y);

        if self.is_radial_menu_open() {
            self.update_radial_menu_hover(screen_x as f64, screen_y as f64);
            return;
        }

        if self.is_color_picker_popup_open() {
            if self.color_picker_popup_is_dragging()
                && let Some(layout) = self.color_picker_popup_layout()
            {
                let fx = screen_x as f64;
                let fy = screen_y as f64;
                let norm_x = ((fx - layout.gradient_x) / layout.gradient_w).clamp(0.0, 1.0);
                let norm_y = ((fy - layout.gradient_y) / layout.gradient_h).clamp(0.0, 1.0);
                self.color_picker_popup_set_from_gradient(norm_x, norm_y);
            }
            return;
        }

        if self.is_board_picker_open() {
            if self.board_picker_is_page_dragging() {
                self.board_picker_update_page_drag_from_pointer(screen_x, screen_y);
            } else if self.board_picker_is_dragging() {
                self.board_picker_update_drag_from_pointer(screen_x, screen_y);
            } else {
                self.update_board_picker_hover_from_pointer(screen_x, screen_y);
            }
            return;
        }

        if self.is_properties_panel_open() {
            if self.properties_panel_layout().is_none() {
                return;
            }
            self.update_properties_panel_hover_from_pointer(screen_x, screen_y);
            return;
        }

        if let DrawingState::ResizingText {
            shape_id,
            base_x,
            size,
            ..
        } = &self.state
        {
            let new_width = self.clamp_text_wrap_width(*base_x, canvas_x, *size);
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
            let dx = canvas_x - *start_x;
            let dy = canvas_y - *start_y;
            if dx.abs() >= TEXT_CLICK_DRAG_THRESHOLD || dy.abs() >= TEXT_CLICK_DRAG_THRESHOLD {
                let tool = *tool;
                if tool != Tool::Highlight && tool != Tool::Select {
                    let mut points = vec![(*start_x, *start_y)];
                    let mut point_thicknesses = vec![self.current_thickness as f32];
                    if tool == Tool::Pen || tool == Tool::Marker || tool == Tool::Eraser {
                        points.push((canvas_x, canvas_y));
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
                    self.update_provisional_dirty(canvas_x, canvas_y);
                    self.needs_redraw = true;
                }
            }
            return;
        }

        if let DrawingState::MovingSelection { last_x, last_y, .. } = &self.state {
            let dx = canvas_x - *last_x;
            let dy = canvas_y - *last_y;
            if (dx != 0 || dy != 0)
                && self.apply_translation_to_selection(dx, dy)
                && let DrawingState::MovingSelection {
                    last_x,
                    last_y,
                    moved,
                    ..
                } = &mut self.state
            {
                *last_x = canvas_x;
                *last_y = canvas_y;
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
            let dx = canvas_x - *start_x;
            let dy = canvas_y - *start_y;
            let handle = *handle;
            let original_bounds = *original_bounds;
            let snapshots = Arc::clone(snapshots);
            self.apply_selection_resize(handle, &original_bounds, dx, dy, snapshots.as_ref());
            self.needs_redraw = true;
            return;
        }

        if matches!(self.state, DrawingState::Selecting { .. }) {
            self.update_provisional_dirty(canvas_x, canvas_y);
            self.needs_redraw = true;
            return;
        }

        if self.is_context_menu_open() {
            self.update_context_menu_hover_from_pointer(screen_x, screen_y);
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
                points.push((canvas_x, canvas_y));
                point_thicknesses.push(self.current_thickness as f32);
            }
            drawing = true;
        }

        if drawing {
            self.update_provisional_dirty(canvas_x, canvas_y);
            self.needs_redraw = true;
        } else if self.eraser_mode == EraserMode::Stroke
            && self.active_tool() == Tool::Eraser
            && matches!(self.state, DrawingState::Idle)
        {
            self.needs_redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::input::BOARD_ID_WHITEBOARD;
    use crate::input::state::test_support::make_test_input_state;

    #[test]
    fn on_mouse_motion_wrapper_preserves_screen_coords_from_canvas_transform() {
        let mut state = make_test_input_state();
        state.switch_board(BOARD_ID_WHITEBOARD);
        assert!(state.boards.active_frame_mut().set_view_offset(100, 200));

        state.on_mouse_motion(130, 240);

        assert_eq!(state.pointer_position(), (30, 40));
        assert_eq!(state.canvas_pointer_position(), (130, 240));
    }
}
