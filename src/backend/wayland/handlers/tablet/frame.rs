use log::{debug, info};

use crate::backend::wayland::state::WaylandState;
use crate::input::MouseButton;

impl WaylandState {
    /// Queue a tablet motion axis update until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_motion(&mut self, x: f64, y: f64) {
        self.pending_stylus_frame.motion = Some((x, y));
    }

    /// Queue a tablet pressure axis update until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_pressure(&mut self, pressure: u32) {
        self.pending_stylus_frame.pressure = Some(pressure);
    }

    /// Queue logical tip contact until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_down(&mut self) {
        self.pending_stylus_frame.down = true;
    }

    /// Queue logical tip release until the enclosing tablet frame commits.
    pub(super) fn queue_stylus_up(&mut self) {
        self.pending_stylus_frame.up = true;
    }

    /// Commit coalesced tablet tool state.
    ///
    /// Invariant: drawing samples are appended only after applying the pressure
    /// update from the same committed tablet frame.
    pub(super) fn commit_pending_stylus_frame(&mut self) {
        let pending = std::mem::take(&mut self.pending_stylus_frame);
        if pending.is_empty() {
            return;
        }

        if let Some(pressure) = pending.pressure {
            self.apply_committed_stylus_pressure(pressure);
        }

        if let Some((x, y)) = pending.motion {
            self.commit_stylus_motion_sample(x, y);
        }

        if pending.down {
            self.commit_stylus_down();
        }

        if pending.pressure.is_some()
            && pending.motion.is_none()
            && !pending.down
            && self.stylus_tip_down
        {
            self.commit_stylus_motion_sample_at_current_position();
        }

        if pending.up {
            self.commit_stylus_up();
        }
    }

    pub(super) fn current_or_pending_stylus_position(&self) -> (f64, f64) {
        self.pending_stylus_frame
            .motion
            .or(self.stylus_last_pos)
            .unwrap_or_else(|| {
                let (x, y) = self.current_mouse();
                (x as f64, y as f64)
            })
    }

    fn apply_committed_stylus_pressure(&mut self, pressure: u32) {
        if pressure == 0 {
            debug!("Stylus pressure reported 0; deferring to peak/base");
            return;
        }

        let p01 = (pressure as f64) / 65535.0;
        crate::input::tablet::apply_pressure_to_state(
            p01,
            &mut self.input_state,
            self.tablet_settings,
        );
        self.stylus_pressure_thickness = Some(self.input_state.current_thickness);
        self.record_stylus_peak(self.input_state.current_thickness);
    }

    fn commit_stylus_motion_sample(&mut self, x: f64, y: f64) {
        let previous_hover_cursor_pos = self.stylus_hover_cursor_position();
        self.set_current_mouse(x as i32, y as i32);
        self.stylus_last_pos = Some((x, y));
        let (wx, wy) = self.zoomed_world_coords(x, y);
        self.input_state
            .on_mouse_motion_with_canvas(x.round() as i32, y.round() as i32, wx, wy);
        let next_hover_cursor_pos = self.stylus_hover_cursor_position();
        self.mark_stylus_hover_cursor_dirty(previous_hover_cursor_pos, next_hover_cursor_pos);
        if self.stylus_tip_down {
            self.stylus_pressure_thickness = Some(self.input_state.current_thickness);
            self.record_stylus_peak(self.input_state.current_thickness);
        }
    }

    fn commit_stylus_motion_sample_at_current_position(&mut self) {
        let (x, y) = self.current_stylus_position();
        self.commit_stylus_motion_sample(x, y);
    }

    fn commit_stylus_down(&mut self) {
        if !self.stylus_on_overlay {
            return;
        }

        let hover_cursor_pos = self.stylus_hover_cursor_position();
        let (x, y) = self.current_stylus_position();
        self.set_current_mouse(x as i32, y as i32);
        self.stylus_tip_down = true;
        self.mark_stylus_hover_cursor_dirty(hover_cursor_pos, None);
        info!(
            "Stylus DOWN at ({}, {})",
            self.current_mouse().0,
            self.current_mouse().1
        );
        let screen_x = self.current_mouse().0;
        let screen_y = self.current_mouse().1;
        let (wx, wy) = self.zoomed_world_coords(x, y);
        self.input_state
            .on_mouse_press_with_canvas(MouseButton::Left, screen_x, screen_y, wx, wy);
        let base_thickness = self.input_state.current_thickness;
        self.stylus_base_thickness = Some(base_thickness);
        self.stylus_pressure_thickness = Some(base_thickness);
        self.record_stylus_peak(base_thickness);
        self.input_state.needs_redraw = true;
    }

    fn commit_stylus_up(&mut self) {
        if !self.stylus_on_overlay {
            return;
        }

        self.stylus_tip_down = false;
        let final_thick = self
            .stylus_peak_thickness
            .or(self.stylus_pressure_thickness)
            .or(self.stylus_base_thickness);
        if let Some(thick) = final_thick {
            self.input_state
                .set_pressure_thickness_for_active_tool(thick);
            self.stylus_base_thickness = Some(thick);
        }
        self.stylus_pressure_thickness = None;
        self.stylus_peak_thickness = None;
        info!(
            "Stylus UP at ({}, {})",
            self.current_mouse().0,
            self.current_mouse().1
        );
        let (x, y) = self.current_stylus_position();
        self.set_current_mouse(x as i32, y as i32);
        let screen_x = self.current_mouse().0;
        let screen_y = self.current_mouse().1;
        let (wx, wy) = self.zoomed_world_coords(x, y);
        self.input_state.on_mouse_release_with_canvas(
            MouseButton::Left,
            screen_x,
            screen_y,
            wx,
            wy,
        );
        let hover_cursor_pos = self.stylus_hover_cursor_position();
        self.mark_stylus_hover_cursor_dirty(None, hover_cursor_pos);
        self.input_state.needs_redraw = true;
    }

    fn current_stylus_position(&self) -> (f64, f64) {
        self.current_or_pending_stylus_position()
    }
}
