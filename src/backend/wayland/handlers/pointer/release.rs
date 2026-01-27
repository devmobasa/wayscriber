use log::debug;
use smithay_client_toolkit::seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent};

use crate::backend::wayland::state::drag_log;
use crate::input::MouseButton;

use super::*;

impl WaylandState {
    pub(super) fn handle_pointer_release(
        &mut self,
        event: &PointerEvent,
        on_toolbar: bool,
        inline_active: bool,
        button: u32,
    ) {
        // Swallow releases after modal clicks (e.g., palette dismiss)
        if self.take_suppress_next_release() {
            return;
        }

        // Block pointer input when modal overlays are active
        if self.input_state.command_palette_open || self.input_state.tour_active {
            // For command palette, press handles the click - release is a no-op
            return;
        }

        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "pointer release: button={}, on_toolbar={}, inline_active={}, drag_active={}, toolbar_dragging={}, pointer_over_toolbar={}",
                button,
                on_toolbar,
                inline_active,
                self.is_move_dragging(),
                self.toolbar_dragging(),
                self.pointer_over_toolbar()
            );
        }
        if inline_active {
            if button == BTN_LEFT && self.inline_toolbar_release(event.position) {
                drag_log(format!(
                    "pointer release: inline handled, pos=({:.3}, {:.3}), drag_active={}, pointer_over_toolbar={}",
                    event.position.0,
                    event.position.1,
                    self.is_move_dragging(),
                    self.pointer_over_toolbar()
                ));
                self.unlock_pointer();
                return;
            }
            if self.pointer_over_toolbar() || self.toolbar_dragging() {
                drag_log(format!(
                    "pointer release: inline end drag, pos=({:.3}, {:.3}), drag_active={}, pointer_over_toolbar={}",
                    event.position.0,
                    event.position.1,
                    self.is_move_dragging(),
                    self.pointer_over_toolbar()
                ));
                self.end_toolbar_move_drag();
                self.unlock_pointer();
                return;
            }
        }
        if on_toolbar || self.pointer_over_toolbar() {
            if button == BTN_LEFT {
                self.set_toolbar_dragging(false);
            }
            drag_log(format!(
                "pointer release: toolbar end drag, pos=({:.3}, {:.3}), drag_active={}, pointer_over_toolbar={}",
                event.position.0,
                event.position.1,
                self.is_move_dragging(),
                self.pointer_over_toolbar()
            ));
            self.end_toolbar_move_drag();
            self.unlock_pointer();
            return;
        }
        // End move drag if released on the main surface
        if button == BTN_LEFT && self.is_move_dragging() {
            self.set_toolbar_dragging(false);
            drag_log(format!(
                "pointer release: main surface end drag, pos=({:.3}, {:.3})",
                event.position.0,
                event.position.1
            ));
            self.end_toolbar_move_drag();
            self.unlock_pointer();
            return;
        }
        debug!("Button {} released", button);
        if self.zoom.active && button == BTN_MIDDLE {
            if self.zoom.panning {
                self.zoom.stop_pan();
                self.input_state.dirty_tracker.mark_full();
                self.input_state.needs_redraw = true;
            }
            return;
        }

        let mb = match button {
            BTN_LEFT => MouseButton::Left,
            BTN_MIDDLE => MouseButton::Middle,
            BTN_RIGHT => MouseButton::Right,
            _ => return,
        };

        // Check for toast click before other handling (toast uses screen coords)
        if mb == MouseButton::Left {
            let screen_x = event.position.0 as i32;
            let screen_y = event.position.1 as i32;
            let (hit, action) = self.input_state.check_toast_click(screen_x, screen_y);
            if hit {
                if let Some(action) = action {
                    self.input_state.handle_action(action);
                }
                return;
            }
        }

        let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
        self.input_state.on_mouse_release(mb, wx, wy);
        self.input_state.needs_redraw = true;
    }
}
