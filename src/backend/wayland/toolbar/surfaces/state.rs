use smithay_client_toolkit::{compositor::CompositorState, shell::WaylandSurface};
use wayland_client::protocol::wl_output;

use super::structs::ToolbarSurface;
use crate::backend::wayland::overlay_passthrough::set_surface_clickthrough;
use crate::backend::wayland::toolbar::hit::{focus_hover_point, focused_event, next_focus_index};
use crate::ui::toolbar::ToolbarEvent;

impl ToolbarSurface {
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_focus(&mut self) {
        if self.focus_index.is_some() {
            self.focus_index = None;
            self.dirty = true;
        }
    }

    pub fn focus_next(&mut self, reverse: bool) -> bool {
        let next = next_focus_index(&self.hit_regions, self.focus_index, reverse);
        if next != self.focus_index {
            self.focus_index = next;
            self.dirty = true;
            return true;
        }
        false
    }

    pub fn focused_hover(&self) -> Option<(f64, f64)> {
        focus_hover_point(&self.hit_regions, self.focus_index)
    }

    pub fn focused_event(&self) -> Option<ToolbarEvent> {
        focused_event(&self.hit_regions, self.focus_index)
    }

    pub fn needs_render(&self) -> bool {
        self.configured && self.dirty && self.width > 0 && self.height > 0
    }

    pub fn set_suppressed(&mut self, compositor: &CompositorState, suppressed: bool) {
        if self.suppressed == suppressed {
            return;
        }
        self.suppressed = suppressed;
        if let Some(surface) = self.wl_surface.as_ref() {
            set_surface_clickthrough(compositor, surface, suppressed, None);
        }
        self.hit_regions.clear();
        self.dirty = true;
    }

    pub fn set_logical_size(&mut self, size: (u32, u32)) {
        self.logical_size = size;
    }

    pub fn set_margins(&mut self, top: i32, right: i32, bottom: i32, left: i32) {
        let next = (top, right, bottom, left);
        if self.margin == next {
            return;
        }
        self.margin = next;
        if let Some(layer) = self.layer_surface.as_ref() {
            layer.set_margin(self.margin.0, self.margin.1, self.margin.2, self.margin.3);
            // Commit immediately so the margin change takes effect.
            layer.wl_surface().commit();
        }
    }

    pub fn set_scale(&mut self, scale: i32) {
        let scale = scale.max(1);
        if self.scale != scale {
            self.scale = scale;
            self.pool = None;
            if let Some(layer) = self.layer_surface.as_mut() {
                let _ = layer.set_buffer_scale(scale as u32);
            } else if let Some(surface) = self.wl_surface.as_ref() {
                surface.set_buffer_scale(scale);
            }
            self.dirty = true;
        }
    }

    pub fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        if output.is_some() {
            self.set_scale(scale);
        }
    }
}
