use log::debug;
use smithay_client_toolkit::seat::pointer::PointerEvent;
use wayland_client::Connection;

use crate::backend::wayland::state::PerfInputSource;
use crate::backend::wayland::state::drag_log;
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
        if self.input_state.eyedropper_is_active() {
            let inline_hover = !on_toolbar
                && self.inline_toolbars_active()
                && self.toolbar.is_visible()
                && self.inline_toolbar_motion(event.position);
            let screen_position = if on_toolbar {
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            } else {
                Some(event.position)
            };
            if let Some((x, y)) = screen_position {
                self.set_current_mouse(x.round() as i32, y.round() as i32);
                self.update_eyedropper_hover(x, y);
            }
            self.update_pointer_cursor(
                on_toolbar || inline_hover || self.pointer_over_toolbar(),
                conn,
            );
            return;
        }

        if self.is_move_dragging()
            && let Some(kind) = self.active_move_drag_kind()
        {
            drag_log(format!(
                "pointer motion: drag_active kind={:?}, pos=({:.3}, {:.3}), on_toolbar={}, inline_active={}",
                kind, event.position.0, event.position.1, on_toolbar, inline_active
            ));
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
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            return;
        }
        // An open radial menu owns pointer motion everywhere on screen:
        // flick sampling and wedge hover must keep working when the pointer
        // crosses a toolbar region, so bypass the toolbar gates below until
        // the menu closes.
        if self.input_state.is_radial_menu_open() && !self.is_move_dragging() {
            let screen_position = if on_toolbar {
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            } else {
                Some(event.position)
            };
            if let Some((sx, sy)) = screen_position {
                self.set_current_mouse(sx.round() as i32, sy.round() as i32);
                let (wx, wy) = self.zoomed_world_coords(sx, sy);
                self.input_state.update_pointer_positions(
                    sx.round() as i32,
                    sy.round() as i32,
                    wx,
                    wy,
                );
                self.input_state.on_mouse_motion_with_canvas(
                    sx.round() as i32,
                    sy.round() as i32,
                    wx,
                    wy,
                );
                self.update_pointer_cursor(false, conn);
                return;
            }
        }
        if inline_active && self.inline_toolbar_motion(event.position) {
            self.update_pointer_cursor(true, conn);
            return;
        }
        if on_toolbar {
            self.set_pointer_over_toolbar(true);
            if let Some((sx, sy)) =
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            {
                self.set_current_mouse(sx as i32, sy as i32);
                let (wx, wy) = self.zoomed_world_coords(sx, sy);
                self.input_state
                    .update_pointer_positions(sx as i32, sy as i32, wx, wy);
            }
            let evt = self.toolbar.pointer_motion(&event.surface, event.position);
            if self.toolbar_dragging() {
                // Use move_drag_intent if pointer_motion didn't return an intent
                // This allows dragging to continue when mouse moves outside hit region
                let intent =
                    evt.or_else(|| self.move_drag_intent(event.position.0, event.position.1));
                if let Some(intent) = intent {
                    let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                    self.handle_toolbar_event(evt, None, None);
                }
            } else {
                self.toolbar.mark_dirty();
            }
            if inline_active {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            self.refresh_keyboard_interactivity();
            self.update_pointer_cursor(true, conn);
            return;
        }
        if self.pointer_over_toolbar() {
            self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
            let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
            self.input_state.update_pointer_positions(
                event.position.0.round() as i32,
                event.position.1.round() as i32,
                wx,
                wy,
            );
            let evt = self.toolbar.pointer_motion(&event.surface, event.position);
            if self.toolbar_dragging() {
                // Use move_drag_intent if pointer_motion didn't return an intent
                // This allows dragging to continue when mouse moves outside hit region
                let intent =
                    evt.or_else(|| self.move_drag_intent(event.position.0, event.position.1));
                if let Some(intent) = intent {
                    let evt = intent_to_event(intent, self.toolbar.last_snapshot());
                    self.handle_toolbar_event(evt, None, None);
                }
            } else {
                self.toolbar.mark_dirty();
            }
            if inline_active {
                self.input_state.dirty_tracker.mark_full();
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
                self.handle_toolbar_event(evt, None, None);
                self.toolbar.mark_dirty();
                self.input_state.dirty_tracker.mark_full();
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
            self.sync_input_zoom_state();
            let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
            self.input_state.update_pointer_positions(
                event.position.0.round() as i32,
                event.position.1.round() as i32,
                wx,
                wy,
            );
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }
        if self.board_panning_active() {
            self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
            let (dx, dy) = self.update_board_pan_position(event.position.0, event.position.1);
            let _ = self.pan_board_by_screen_delta(dx, dy);
            let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
            self.input_state.update_pointer_positions(
                event.position.0.round() as i32,
                event.position.1.round() as i32,
                wx,
                wy,
            );
            return;
        }
        // Capture the pre-motion pointer so the idle tool-preview bubble can
        // damage its old position; the new position is the incoming event.
        let prev_mouse = self.current_mouse();
        let next_mouse = (event.position.0 as i32, event.position.1 as i32);
        self.set_current_mouse(next_mouse.0, next_mouse.1);
        // The command palette owns hover rendering (including shortcut-action
        // tooltips), so keep its screen-space pointer cache and redraw current
        // even though normal canvas motion remains blocked by the modal.
        if self.input_state.command_palette_open {
            let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
            self.input_state.update_pointer_positions(
                event.position.0.round() as i32,
                event.position.1.round() as i32,
                wx,
                wy,
            );
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }
        // Block normal pointer motion while the tour modal is active.
        if self.input_state.tour_active {
            return;
        }
        let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
        self.input_state.update_pointer_positions(
            event.position.0.round() as i32,
            event.position.1.round() as i32,
            wx,
            wy,
        );
        self.input_state.on_mouse_motion_with_canvas(
            event.position.0.round() as i32,
            event.position.1.round() as i32,
            wx,
            wy,
        );
        // Idle pointer motion otherwise only refreshes cached coordinates, so
        // the trailing tool-preview bubble would freeze at its previous spot
        // (stroke/eraser hover redraws are handled above). Damage the old and
        // new bubble footprints so it follows the cursor. Evaluated after the
        // motion so a drag that started a stroke (state -> Drawing) is not
        // treated as an eligible preview.
        self.mark_mouse_tool_preview_dirty(prev_mouse, next_mouse);
        self.record_perf_input_sample(
            PerfInputSource::Pointer,
            event.position.0.round() as i32,
            event.position.1.round() as i32,
            wx,
            wy,
            false,
        );
    }
}
