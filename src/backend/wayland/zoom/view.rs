use log::info;

use crate::input::InputState;

use super::state::ZoomState;
use super::{MAX_ZOOM_SCALE, MIN_ZOOM_SCALE};

impl ZoomState {
    pub fn zoom_at_screen_point(
        &mut self,
        factor: f64,
        screen_x: f64,
        screen_y: f64,
        screen_width: u32,
        screen_height: u32,
    ) -> bool {
        let old_scale = self.scale;
        let mut new_scale = old_scale * factor;
        new_scale = new_scale.clamp(MIN_ZOOM_SCALE, MAX_ZOOM_SCALE);
        if (new_scale - old_scale).abs() < f64::EPSILON {
            return false;
        }
        let (world_x, world_y) = self.screen_to_world(screen_x, screen_y);
        self.scale = new_scale;
        self.view_offset.0 = world_x - (screen_x / new_scale);
        self.view_offset.1 = world_y - (screen_y / new_scale);
        self.clamp_offsets(screen_width, screen_height);
        true
    }

    pub fn screen_to_world(&self, screen_x: f64, screen_y: f64) -> (f64, f64) {
        (
            self.view_offset.0 + (screen_x / self.scale),
            self.view_offset.1 + (screen_y / self.scale),
        )
    }

    pub fn clamp_offsets(&mut self, screen_width: u32, screen_height: u32) {
        let width = screen_width as f64;
        let height = screen_height as f64;
        let visible_w = width / self.scale.max(MIN_ZOOM_SCALE);
        let visible_h = height / self.scale.max(MIN_ZOOM_SCALE);
        let max_x = (width - visible_w).max(0.0);
        let max_y = (height - visible_h).max(0.0);
        self.view_offset.0 = self.view_offset.0.clamp(0.0, max_x);
        self.view_offset.1 = self.view_offset.1.clamp(0.0, max_y);
    }

    pub fn start_pan(&mut self, screen_x: f64, screen_y: f64) {
        self.panning = true;
        self.last_pan_pos = (screen_x, screen_y);
    }

    pub fn stop_pan(&mut self) {
        self.panning = false;
    }

    pub fn pan_by_screen_delta(&mut self, dx: f64, dy: f64, screen_width: u32, screen_height: u32) {
        if self.scale <= MIN_ZOOM_SCALE {
            return;
        }
        self.view_offset.0 -= dx / self.scale;
        self.view_offset.1 -= dy / self.scale;
        self.clamp_offsets(screen_width, screen_height);
    }

    pub fn update_pan_position(&mut self, screen_x: f64, screen_y: f64) -> (f64, f64) {
        let (last_x, last_y) = self.last_pan_pos;
        self.last_pan_pos = (screen_x, screen_y);
        (screen_x - last_x, screen_y - last_y)
    }

    /// Drop zoom image if the surface size no longer matches.
    pub fn handle_resize(
        &mut self,
        phys_width: u32,
        phys_height: u32,
        input_state: &mut InputState,
    ) {
        if let Some(img) = &self.image
            && (img.width != phys_width || img.height != phys_height)
        {
            info!("Surface resized; clearing zoom image");
            self.deactivate(input_state);
        }
    }
}
