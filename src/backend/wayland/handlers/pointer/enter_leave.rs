use log::debug;
use smithay_client_toolkit::seat::pointer::PointerEvent;
use wayland_client::Connection;

use crate::input::{EraserMode, Tool};

use super::*;

impl WaylandState {
    pub(super) fn handle_pointer_enter(
        &mut self,
        conn: &Connection,
        event: &PointerEvent,
        on_toolbar: bool,
        inline_active: bool,
    ) {
        debug!(
            "Pointer entered at ({}, {}), on_toolbar={}, is_move_dragging={}",
            event.position.0,
            event.position.1,
            on_toolbar,
            self.is_move_dragging()
        );
        self.set_pointer_focus(true);
        self.set_pointer_over_toolbar(on_toolbar);
        if on_toolbar {
            if let Some((sx, sy)) =
                self.toolbar_surface_screen_coords(&event.surface, event.position)
            {
                self.set_current_mouse(sx as i32, sy as i32);
                let (wx, wy) = self.zoomed_world_coords(sx, sy);
                self.input_state.update_pointer_position(wx, wy);
            } else {
                self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
            }
            // Ensure pointer-driven visuals (e.g. eraser hover) update once on enter.
            self.input_state.needs_redraw = true;
        }
        if !on_toolbar {
            self.set_current_mouse(event.position.0 as i32, event.position.1 as i32);
            let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
            self.input_state.update_pointer_position(wx, wy);
            if self.input_state.eraser_mode == EraserMode::Stroke
                && self.input_state.active_tool() == Tool::Eraser
            {
                self.input_state.needs_redraw = true;
            }
        }
        self.update_pointer_cursor(on_toolbar, conn);
        if inline_active {
            self.inline_toolbar_motion(event.position);
        }
    }

    pub(super) fn handle_pointer_leave(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        inline_active: bool,
    ) {
        debug!(
            "Pointer left surface: on_toolbar={}, is_move_dragging={}",
            on_toolbar,
            self.is_move_dragging()
        );
        self.set_pointer_focus(false);
        if on_toolbar {
            self.set_pointer_over_toolbar(false);
            self.toolbar.pointer_leave(&event.surface);
            // Don't clear drag state if we're in a move drag - the user may be
            // dragging the toolbar and their pointer left the toolbar surface.
            // The drag will continue on the main surface.
            if !self.is_move_dragging() {
                debug!("Clearing toolbar drag state on leave");
                self.set_toolbar_dragging(false);
                self.end_toolbar_move_drag();
            } else {
                debug!("Preserving move drag state on toolbar leave");
            }
            self.toolbar.mark_dirty();
            // Ensure pointer-driven visuals (e.g. eraser hover) update once on leave.
            self.input_state.needs_redraw = true;
        }
        if !on_toolbar
            && self.input_state.eraser_mode == EraserMode::Stroke
            && self.input_state.active_tool() == Tool::Eraser
        {
            self.input_state.needs_redraw = true;
        }
        if inline_active {
            self.inline_toolbar_leave();
        }
        if (on_toolbar || inline_active) && !self.is_move_dragging() {
            self.end_toolbar_move_drag();
        }
        self.current_pointer_shape = None;
        self.cursor_hidden = false;
    }
}
