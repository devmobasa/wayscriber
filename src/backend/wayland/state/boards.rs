use crate::config::BoardsConfig;
use crate::input::DrawingState;

use super::WaylandState;

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_board_config_update(&mut self, boards: BoardsConfig) {
        self.config.boards = Some(boards);
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save board config: {}", err);
        }
    }

    pub(in crate::backend::wayland) fn board_view_offset(&self) -> (f64, f64) {
        if self.input_state.board_is_transparent() || !self.input_state.boards.pan_enabled() {
            (0.0, 0.0)
        } else {
            let (x, y) = self.input_state.boards.active_frame().view_offset();
            (x as f64, y as f64)
        }
    }

    pub(in crate::backend::wayland) fn canvas_view_origin(&self) -> (f64, f64) {
        let (board_x, board_y) = self.board_view_offset();
        if self.zoom.active {
            (
                board_x + self.zoom.view_offset.0,
                board_y + self.zoom.view_offset.1,
            )
        } else {
            (board_x, board_y)
        }
    }

    pub(in crate::backend::wayland) fn canvas_transform_active(&self) -> bool {
        self.zoom.active
            || (self.input_state.boards.pan_enabled()
                && !self.input_state.board_is_transparent()
                && self.input_state.boards.active_frame().view_offset() != (0, 0))
    }

    pub(in crate::backend::wayland) fn canvas_world_coords(
        &self,
        screen_x: f64,
        screen_y: f64,
    ) -> (i32, i32) {
        let (board_x, board_y) = self.board_view_offset();
        if self.zoom.active {
            let (zoom_x, zoom_y) = self.zoom.screen_to_world(screen_x, screen_y);
            (
                (board_x + zoom_x).round() as i32,
                (board_y + zoom_y).round() as i32,
            )
        } else {
            (
                (board_x + screen_x).round() as i32,
                (board_y + screen_y).round() as i32,
            )
        }
    }

    pub(in crate::backend::wayland) fn can_start_board_pan(&self) -> bool {
        self.input_state.boards.pan_enabled()
            && !self.input_state.board_is_transparent()
            && !self.zoom.active
            && !self.input_state.tour_active
            && !self.input_state.command_palette_open
            && !self.input_state.is_board_picker_open()
            && !self.input_state.is_color_picker_popup_open()
            && !self.input_state.is_context_menu_open()
            && !self.input_state.is_properties_panel_open()
            && !self.input_state.is_radial_menu_open()
            && matches!(self.input_state.state, DrawingState::Idle)
    }

    pub(in crate::backend::wayland) fn start_board_pan(&mut self, screen_x: f64, screen_y: f64) {
        self.data.board_panning = true;
        self.data.board_pan_last_pos = (screen_x, screen_y);
    }

    pub(in crate::backend::wayland) fn stop_board_pan(&mut self) {
        self.data.board_panning = false;
    }

    pub(in crate::backend::wayland) fn board_panning_active(&self) -> bool {
        self.data.board_panning
    }

    pub(in crate::backend::wayland) fn board_pan_key_held(&self) -> bool {
        self.data.board_pan_key_held
    }

    pub(in crate::backend::wayland) fn set_board_pan_key_held(&mut self, held: bool) {
        self.data.board_pan_key_held = held;
    }

    pub(in crate::backend::wayland) fn pan_board_by_screen_delta(
        &mut self,
        dx: f64,
        dy: f64,
    ) -> bool {
        if self.input_state.board_is_transparent() || !self.input_state.boards.pan_enabled() {
            return false;
        }
        let dx = dx.round() as i32;
        let dy = dy.round() as i32;
        if dx == 0 && dy == 0 {
            return false;
        }
        let changed = self
            .input_state
            .boards
            .active_frame_mut()
            .pan_view_by(-dx, -dy);
        if changed {
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            self.input_state.mark_session_dirty();
        }
        changed
    }

    pub(in crate::backend::wayland) fn update_board_pan_position(
        &mut self,
        screen_x: f64,
        screen_y: f64,
    ) -> (f64, f64) {
        let (last_x, last_y) = self.data.board_pan_last_pos;
        self.data.board_pan_last_pos = (screen_x, screen_y);
        (screen_x - last_x, screen_y - last_y)
    }

    pub(in crate::backend::wayland) fn should_capture_space_for_board_pan(&self) -> bool {
        self.input_state.boards.pan_enabled()
            && !self.input_state.board_is_transparent()
            && !self.zoom.active
            && !self.input_state.tour_active
            && !self.input_state.show_help
            && !self.input_state.command_palette_open
            && !self.input_state.is_board_picker_open()
            && !self.input_state.is_color_picker_popup_open()
            && !self.input_state.is_context_menu_open()
            && !self.input_state.is_properties_panel_open()
            && !self.input_state.is_radial_menu_open()
            && !self.pointer_over_toolbar()
            && self.toolbar_focus_target().is_none()
            && matches!(self.input_state.state, DrawingState::Idle)
    }
}
