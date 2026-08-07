use std::time::Instant;

use wayland_client::protocol::wl_surface;

use super::structs::ToolbarSurfaceManager;
use crate::backend::wayland::toolbar_intent::ToolbarIntent;
use crate::ui::toolbar::ToolbarEvent;

impl ToolbarSurfaceManager {
    pub fn pointer_press(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<(ToolbarIntent, bool)> {
        if self.top.is_surface(surface) {
            return self.top.hit_at(position.0, position.1);
        }
        None
    }

    /// The quick-color slot under the pointer, if the press landed on a
    /// palette swatch. Secondary click reads the same hit regions as the
    /// primary path, so the recolor gesture cannot drift from what is drawn.
    pub fn quick_color_slot_at(
        &self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<usize> {
        if self.top.is_surface(surface) {
            return self.top.quick_color_slot_at(position.0, position.1);
        }
        None
    }

    pub fn pointer_motion(
        &mut self,
        surface: &wl_surface::WlSurface,
        position: (f64, f64),
    ) -> Option<ToolbarIntent> {
        if self.top.is_surface(surface) {
            if self.top_hover != Some(position) {
                // Reset hover start time when position changes
                if self.top_hover.is_none() {
                    self.top_hover_start = Some(Instant::now());
                }
                self.top_hover = Some(position);
                self.top.set_hover(Some(position));
            }
            return self.top.drag_at(position.0, position.1);
        }
        None
    }

    pub fn pointer_leave(&mut self, surface: &wl_surface::WlSurface) {
        if self.top.is_surface(surface) {
            self.top_hover = None;
            self.top_hover_start = None;
            self.top.set_hover(None);
        }
    }

    /// Whether the surface is the top toolbar surface, i.e. whether keyboard
    /// focus routing applies to it.
    pub fn is_focusable_surface(&self, surface: &wl_surface::WlSurface) -> bool {
        self.top.is_surface(surface)
    }

    pub fn is_hovered(&self) -> bool {
        self.top_hover.is_some()
    }

    pub fn clear_focus(&mut self) {
        self.top.clear_focus();
    }

    pub fn focus_next(&mut self, reverse: bool) -> bool {
        self.top.focus_next(reverse)
    }

    pub fn focused_event(&self) -> Option<ToolbarEvent> {
        self.top.focused_event()
    }
}
