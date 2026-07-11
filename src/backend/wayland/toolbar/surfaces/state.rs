use smithay_client_toolkit::{compositor::CompositorState, shell::WaylandSurface};
use wayland_client::protocol::wl_output;

use super::structs::ToolbarSurface;
use crate::backend::wayland::overlay_passthrough::set_surface_clickthrough;
use crate::backend::wayland::toolbar::hit::{
    focus_hover_point, focused_event, next_focus_index, resolve_focus_index,
};
use crate::ui::toolbar::ToolbarEvent;

impl ToolbarSurface {
    pub fn set_hover(&mut self, hover: Option<(f64, f64)>) {
        if self.hover != hover {
            self.hover = hover;
            self.dirty = true;
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_focus(&mut self) {
        if self.focus_index.is_some() || self.focus_id.is_some() {
            self.focus_index = None;
            self.focus_id = None;
            self.dirty = true;
        }
    }

    pub fn focus_next(&mut self, reverse: bool) -> bool {
        let current = resolve_focus_index(
            &self.hit_regions,
            self.focus_index,
            self.focus_id.as_deref(),
        );
        let next = next_focus_index(&self.hit_regions, current, reverse);
        if next != current {
            self.focus_index = next;
            self.focus_id = next.and_then(|index| self.hit_regions[index].focus_id.clone());
            self.dirty = true;
            return true;
        }
        false
    }

    pub fn focused_hover(&self) -> Option<(f64, f64)> {
        focus_hover_point(
            &self.hit_regions,
            resolve_focus_index(
                &self.hit_regions,
                self.focus_index,
                self.focus_id.as_deref(),
            ),
        )
    }

    pub fn focused_event(&self) -> Option<ToolbarEvent> {
        focused_event(
            &self.hit_regions,
            resolve_focus_index(
                &self.hit_regions,
                self.focus_index,
                self.focus_id.as_deref(),
            ),
        )
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
            set_surface_clickthrough(compositor, surface, suppressed);
        }
        if !suppressed {
            // Unsuppressing restored the full input region; reapply any
            // partial region the content declared.
            self.input_region_dirty = self.input_rects.is_some();
        }
        self.hit_regions.clear();
        self.dirty = true;
    }

    /// Restrict the top surface's input region to the bar band plus any
    /// open popover panels, in surface coordinates; full-surface otherwise.
    pub fn sync_top_input_region(&mut self, snapshot: &crate::ui::toolbar::ToolbarSnapshot) {
        if self.width == 0 || self.height == 0 {
            return;
        }
        let ui_scale = if self.ui_scale.is_finite() {
            self.ui_scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        let rects = crate::backend::wayland::toolbar::view::top::top_input_rects(
            snapshot,
            self.width as f64 / ui_scale,
            self.height as f64 / ui_scale,
        )
        .map(|rects| {
            rects
                .iter()
                .map(|(x, y, w, h)| (x * ui_scale, y * ui_scale, w * ui_scale, h * ui_scale))
                .collect()
        });
        self.set_input_rects(rects);
    }

    /// Declare which surface-local rects should accept input; None means the
    /// whole surface. Applied after the next render via
    /// [`Self::apply_input_region`].
    pub fn set_input_rects(&mut self, rects: Option<Vec<(f64, f64, f64, f64)>>) {
        if self.input_rects != rects {
            self.input_rects = rects;
            self.input_region_dirty = true;
        }
    }

    /// Apply a pending input-region change. Suppression owns the region
    /// while active; the pending change is applied on unsuppress instead.
    pub fn apply_input_region(&mut self, compositor: &CompositorState) {
        if !self.input_region_dirty || self.suppressed {
            return;
        }
        let Some(surface) = self.wl_surface.as_ref() else {
            return;
        };
        match self.input_rects.as_deref() {
            None => surface.set_input_region(None),
            Some(rects) => {
                let Ok(region) = smithay_client_toolkit::compositor::Region::new(compositor) else {
                    return;
                };
                for &(x, y, w, h) in rects {
                    // Round outward so edge pixels stay clickable.
                    let x0 = x.floor() as i32;
                    let y0 = y.floor() as i32;
                    let x1 = (x + w).ceil() as i32;
                    let y1 = (y + h).ceil() as i32;
                    region.add(x0, y0, x1 - x0, y1 - y0);
                }
                surface.set_input_region(Some(region.wl_region()));
            }
        }
        surface.commit();
        self.input_region_dirty = false;
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

    pub fn set_ui_scale(&mut self, scale: f64) {
        // Sanitize scale: handle NaN/Inf and enforce bounds
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 3.0)
        } else {
            1.0
        };
        if (self.ui_scale - scale).abs() > f64::EPSILON {
            self.ui_scale = scale;
            self.dirty = true;
        }
    }

    pub fn maybe_update_scale(&mut self, output: Option<&wl_output::WlOutput>, scale: i32) {
        if output.is_some() {
            self.set_scale(scale);
        }
    }
}
