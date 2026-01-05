use log::debug;
use smithay_client_toolkit::seat::pointer::PointerEvent;
use wayland_client::Connection;

use crate::backend::wayland::toolbar_intent::intent_to_event;

use super::*;

impl WaylandState {
    pub(super) fn handle_pointer_motion(
        &mut self,
        conn: &Connection,
        event: &PointerEvent,
        on_toolbar: bool,
        inline_active: bool,
    ) {
        if self.is_move_dragging()
            && let Some(kind) = self.active_move_drag_kind()
        {
            debug!(
                "Move drag motion: kind={:?}, pos=({}, {}), on_toolbar={}",
                kind, event.position.0, event.position.1, on_toolbar
            );
            // On toolbar surface: coords are toolbar-local, need conversion
            // On main surface: coords are already screen-relative (fullscreen overlay)
            if on_toolbar {
                self.handle_toolbar_move(kind, event.position);
            } else {
                self.handle_toolbar_move_screen(kind, event.position);
            }
            self.toolbar.mark_dirty();
            if inline_active {
                self.input_state.needs_redraw = true;
            }
            return;
        }
        if inline_active && self.inline_toolbar_motion(event.position) {
            self.update_pointer_cursor(true, conn);
            return;
        }
        if on_toolbar {
            self.set_pointer_over_toolbar(true);
            let evt = self.toolbar.pointer_motion(&event.surface, event.position);
            if self.toolbar_dragging() {
                // Use move_drag_intent if pointer_motion didn't return an intent
                // This allows dragging to continue when mouse moves outside hit region
                let intent =
                    evt.or_else(|| self.move_drag_intent(event.position.0, event.position.1));
                if let Some(intent) = intent {
                    let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                    self.handle_toolbar_event(evt);
                }
            } else {
                self.toolbar.mark_dirty();
            }
            if inline_active {
                self.input_state.needs_redraw = true;
            }
            self.refresh_keyboard_interactivity();
            self.update_pointer_cursor(true, conn);
            return;
        }
        if self.pointer_over_toolbar() {
            let evt = self.toolbar.pointer_motion(&event.surface, event.position);
            if self.toolbar_dragging() {
                // Use move_drag_intent if pointer_motion didn't return an intent
                // This allows dragging to continue when mouse moves outside hit region
                let intent =
                    evt.or_else(|| self.move_drag_intent(event.position.0, event.position.1));
                if let Some(intent) = intent {
                    let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                    self.handle_toolbar_event(evt);
                }
            } else {
                self.toolbar.mark_dirty();
            }
            if inline_active {
                self.input_state.needs_redraw = true;
            }
            self.refresh_keyboard_interactivity();
            self.update_pointer_cursor(true, conn);
            return;
        }
        self.update_pointer_cursor(false, conn);
        // Handle move drag that continues on the main surface after leaving toolbar
        if self.is_move_dragging() {
            if let Some(intent) = self.move_drag_intent(event.position.0, event.position.1) {
                let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                self.handle_toolbar_event(evt);
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
            }
            return;
        }
        if self.zoom.panning {
            self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
            let (dx, dy) = self
                .zoom
                .update_pan_position(event.position.0, event.position.1);
            self.zoom
                .pan_by_screen_delta(dx, dy, self.surface.width(), self.surface.height());
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }
        self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
        // Block pointer motion when tour overlay is active
        if self.input_state.tour_active {
            return;
        }
        let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
        self.input_state.update_pointer_position(wx, wy);
        self.input_state.on_mouse_motion(wx, wy);
    }
}
