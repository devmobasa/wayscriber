use log::debug;
use smithay_client_toolkit::seat::pointer::PointerEvent;
use wayland_client::Connection;

use crate::backend::wayland::state::{PerfInputSource, drag_log};
use crate::backend::wayland::toolbar_intent::intent_to_event;

use super::*;

impl WaylandState {
    pub(super) fn handle_pointer_motion(
        &mut self,
        conn: &Connection,
        event: &PointerEvent,
        routed: RoutedInput,
    ) {
        if self.motion_owned_by_screen_modal(conn, routed)
            || self.motion_owned_by_move_drag(conn, event, routed)
            || self.motion_owned_by_radial_menu(conn, routed)
            || self.motion_over_toolbar(conn, event, routed)
            || self.motion_owned_by_pan(conn, routed)
        {
            return;
        }
        if routed.surface == InputSurface::Canvas {
            self.motion_on_canvas(conn, event, routed.screen.unwrap_or(event.position));
        }
    }

    fn motion_owned_by_screen_modal(&mut self, conn: &Connection, routed: RoutedInput) -> bool {
        if self.input_state.region_is_active() {
            if let Some((x, y)) = routed.screen {
                self.pointer
                    .set_position((x.round() as i32, y.round() as i32));
                self.update_region_selection(RegionInputSource::Pointer, x, y);
            }
            self.update_pointer_cursor(
                routed.surface == InputSurface::Toolbar
                    || self.toolbar_chrome.pointer_over_toolbar(),
                conn,
            );
            return true;
        }
        if !self.input_state.eyedropper_is_active() {
            return false;
        }
        let inline_hover = routed.surface == InputSurface::Canvas
            && routed.inline_toolbars
            && routed
                .screen
                .is_some_and(|position| self.inline_toolbar_motion(position));
        if let Some((x, y)) = routed.screen {
            self.pointer
                .set_position((x.round() as i32, y.round() as i32));
            self.update_eyedropper_hover(x, y);
        }
        self.update_pointer_cursor(
            routed.surface == InputSurface::Toolbar
                || inline_hover
                || self.toolbar_chrome.pointer_over_toolbar(),
            conn,
        );
        true
    }

    fn motion_owned_by_move_drag(
        &mut self,
        conn: &Connection,
        event: &PointerEvent,
        routed: RoutedInput,
    ) -> bool {
        if let Some(kind) = self.toolbar_drag.kind() {
            drag_log(|| {
                format!(
                    "pointer motion: drag_active kind={:?}, pos=({:.3}, {:.3}), surface={:?}, inline_active={}",
                    kind,
                    event.position.0,
                    event.position.1,
                    routed.surface,
                    routed.inline_toolbars
                )
            });
            debug!(
                "Move drag motion: kind={:?}, pos=({}, {}), surface={:?}",
                kind, event.position.0, event.position.1, routed.surface
            );
            match routed.surface {
                InputSurface::Toolbar => self.handle_toolbar_move(kind, event.position),
                InputSurface::Canvas => self.handle_toolbar_move_screen(kind, event.position),
                InputSurface::Foreign => return true,
            }
            self.toolbar.mark_dirty();
            if routed.inline_toolbars {
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            return true;
        }
        if !self.toolbar_drag.is_moving() || routed.surface != InputSurface::Canvas {
            return false;
        }
        if let Some(intent) = self.move_drag_intent(event.position.0, event.position.1) {
            let event = intent_to_event(intent, self.toolbar.last_snapshot());
            self.handle_toolbar_event(event, None, None);
            self.toolbar.mark_dirty();
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
        self.update_pointer_cursor(false, conn);
        true
    }

    fn motion_owned_by_radial_menu(&mut self, conn: &Connection, routed: RoutedInput) -> bool {
        if !self.input_state.is_radial_menu_open() || self.toolbar_drag.is_moving() {
            return false;
        }
        let Some((sx, sy)) = routed.screen else {
            return false;
        };
        self.pointer
            .set_position((sx.round() as i32, sy.round() as i32));
        let (wx, wy) = self.zoomed_world_coords(sx, sy);
        self.input_state
            .update_pointer_positions(sx.round() as i32, sy.round() as i32, wx, wy);
        self.input_state
            .on_mouse_motion_with_canvas(sx.round() as i32, sy.round() as i32, wx, wy);
        self.update_pointer_cursor(false, conn);
        true
    }

    fn motion_over_toolbar(
        &mut self,
        conn: &Connection,
        event: &PointerEvent,
        routed: RoutedInput,
    ) -> bool {
        if routed.surface == InputSurface::Canvas
            && routed.inline_toolbars
            && self.inline_toolbar_motion(event.position)
        {
            self.update_pointer_cursor(true, conn);
            return true;
        }
        let toolbar_surface = routed.surface == InputSurface::Toolbar;
        if !toolbar_surface && !self.toolbar_chrome.pointer_over_toolbar() {
            return false;
        }
        self.toolbar_chrome.set_pointer_over_toolbar(true);
        if let Some((sx, sy)) = routed.screen {
            self.pointer.set_position((sx as i32, sy as i32));
            let (wx, wy) = self.zoomed_world_coords(sx, sy);
            self.input_state
                .update_pointer_positions(sx as i32, sy as i32, wx, wy);
        }
        self.update_toolbar_pointer_motion(event);
        if routed.inline_toolbars {
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
        }
        self.refresh_keyboard_interactivity();
        self.update_pointer_cursor(true, conn);
        true
    }

    fn update_toolbar_pointer_motion(&mut self, event: &PointerEvent) {
        let toolbar_event = self.toolbar.pointer_motion(&event.surface, event.position);
        if self.toolbar_drag.item_dragging() {
            let intent =
                toolbar_event.or_else(|| self.move_drag_intent(event.position.0, event.position.1));
            if let Some(intent) = intent {
                let event = intent_to_event(intent, self.toolbar.last_snapshot());
                self.handle_toolbar_event(event, None, None);
            }
        } else {
            self.toolbar.mark_dirty();
        }
    }

    fn motion_owned_by_pan(&mut self, conn: &Connection, routed: RoutedInput) -> bool {
        let Some((sx, sy)) = routed
            .screen
            .filter(|_| routed.surface == InputSurface::Canvas)
        else {
            return false;
        };
        if self.zoom.panning {
            self.pointer.set_position((sx as i32, sy as i32));
            let (dx, dy) = self.zoom.update_pan_position(sx, sy);
            self.zoom
                .pan_by_screen_delta(dx, dy, self.surface.width(), self.surface.height());
            self.sync_input_zoom_state();
            let (wx, wy) = self.zoomed_world_coords(sx, sy);
            self.input_state
                .update_pointer_positions(sx.round() as i32, sy.round() as i32, wx, wy);
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            self.update_pointer_cursor(false, conn);
            return true;
        }
        if !self.pointer.board_pan_active() {
            return false;
        }
        self.pointer.set_position((sx as i32, sy as i32));
        let (dx, dy) = self.pointer.advance_board_pan((sx, sy));
        let _ = self.pan_board_by_screen_delta(dx, dy);
        let (wx, wy) = self.zoomed_world_coords(sx, sy);
        self.input_state
            .update_pointer_positions(sx.round() as i32, sy.round() as i32, wx, wy);
        self.update_pointer_cursor(false, conn);
        true
    }

    fn motion_on_canvas(
        &mut self,
        conn: &Connection,
        event: &PointerEvent,
        screen_position: (f64, f64),
    ) {
        let (sx, sy) = screen_position;
        let previous = self.pointer.position();
        let next = (sx as i32, sy as i32);
        self.pointer.set_position(next);
        if self.input_state.command_palette.open {
            let (wx, wy) = self.zoomed_world_coords(sx, sy);
            self.input_state
                .update_pointer_positions(sx.round() as i32, sy.round() as i32, wx, wy);
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            self.update_pointer_cursor(false, conn);
            return;
        }
        if self.input_state.tour.is_active() {
            self.update_pointer_cursor(false, conn);
            return;
        }
        let (wx, wy) = self.zoomed_world_coords(sx, sy);
        self.input_state
            .update_pointer_positions(sx.round() as i32, sy.round() as i32, wx, wy);
        self.input_state
            .on_mouse_motion_with_canvas(sx.round() as i32, sy.round() as i32, wx, wy);
        self.update_pointer_cursor(false, conn);
        self.mark_mouse_tool_preview_dirty(previous, next);
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
