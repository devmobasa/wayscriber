use std::time::Instant;

use wayland_client::protocol::wl_surface;

use super::structs::ToolbarSurfaceManager;
use crate::backend::wayland::toolbar::ToolbarFocusTarget;
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
        if self.side.is_surface(surface) {
            return self.side.hit_at(position.0, position.1);
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
                self.top.mark_dirty();
            }
            return self.top.drag_at(position.0, position.1);
        }
        if self.side.is_surface(surface) {
            if self.side_hover != Some(position) {
                // Reset hover start time when position changes
                if self.side_hover.is_none() {
                    self.side_hover_start = Some(Instant::now());
                }
                self.side_hover = Some(position);
                self.side.mark_dirty();
            }
            return self.side.drag_at(position.0, position.1);
        }
        None
    }

    pub fn pointer_leave(&mut self, surface: &wl_surface::WlSurface) {
        if self.top.is_surface(surface) {
            self.top_hover = None;
            self.top_hover_start = None;
            self.top.mark_dirty();
        } else if self.side.is_surface(surface) {
            self.side_hover = None;
            self.side_hover_start = None;
            self.side.mark_dirty();
        }
    }

    pub fn focus_target_for_surface(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> Option<ToolbarFocusTarget> {
        if self.top.is_surface(surface) {
            Some(ToolbarFocusTarget::Top)
        } else if self.side.is_surface(surface) {
            Some(ToolbarFocusTarget::Side)
        } else {
            None
        }
    }

    pub fn hovered_target(&self) -> Option<ToolbarFocusTarget> {
        if self.top_hover.is_some() {
            Some(ToolbarFocusTarget::Top)
        } else if self.side_hover.is_some() {
            Some(ToolbarFocusTarget::Side)
        } else {
            None
        }
    }

    pub fn clear_focus(&mut self) {
        self.top.clear_focus();
        self.side.clear_focus();
    }

    pub fn focus_next(&mut self, target: ToolbarFocusTarget, reverse: bool) -> bool {
        match target {
            ToolbarFocusTarget::Top => self.top.focus_next(reverse),
            ToolbarFocusTarget::Side => self.side.focus_next(reverse),
        }
    }

    pub fn focused_event(&self, target: ToolbarFocusTarget) -> Option<ToolbarEvent> {
        match target {
            ToolbarFocusTarget::Top => self.top.focused_event(),
            ToolbarFocusTarget::Side => self.side.focused_event(),
        }
    }
}
