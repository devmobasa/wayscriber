use log::debug;
use smithay_client_toolkit::seat::pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent};
use wayland_client::QueueHandle;

use crate::backend::wayland::toolbar_intent::intent_to_event;
use crate::backend::wayland::state::drag_log;
use crate::input::MouseButton;
use crate::ui::toolbar::ToolbarEvent;

use super::*;

impl WaylandState {
    pub(super) fn handle_pointer_press(
        &mut self,
        _conn: &wayland_client::Connection,
        qh: &QueueHandle<Self>,
        event: &PointerEvent,
        on_toolbar: bool,
        inline_active: bool,
        button: u32,
    ) {
        // Block pointer input when tour is active
        if self.input_state.tour_active {
            return;
        }

        // Handle command palette clicks
        if self.input_state.command_palette_open {
            if button == BTN_LEFT {
                let screen_width = self.surface.width();
                let screen_height = self.surface.height();
                if self.input_state.handle_command_palette_click(
                    event.position.0 as i32,
                    event.position.1 as i32,
                    screen_width,
                    screen_height,
                ) {
                    self.set_suppress_next_release(true);
                }
            }
            return;
        }

        if debug_toolbar_drag_logging_enabled() {
            debug!(
                "pointer press: button={}, on_toolbar={}, inline_active={}, drag_active={}",
                button,
                on_toolbar,
                inline_active,
                self.is_move_dragging()
            );
        }
        if inline_active {
            if button == BTN_LEFT && self.inline_toolbar_press(event.position) {
                drag_log(format!(
                    "pointer press: inline handled, drag_active={}, pos=({:.3}, {:.3}), surface={}",
                    self.toolbar_dragging(),
                    event.position.0,
                    event.position.1,
                    surface_id(&event.surface)
                ));
                if self.toolbar_dragging() {
                    self.lock_pointer_for_drag(qh, &event.surface);
                }
                return;
            }
            if self.pointer_over_toolbar() {
                return;
            }
        }
        if on_toolbar {
            if button == BTN_LEFT
                && let Some((intent, drag)) =
                    self.toolbar.pointer_press(&event.surface, event.position)
            {
                let toolbar_event = intent_to_event(intent, self.toolbar.last_snapshot());
                if matches!(
                    toolbar_event,
                    ToolbarEvent::MoveTopToolbar { .. } | ToolbarEvent::MoveSideToolbar { .. }
                ) && drag
                {
                    self.lock_pointer_for_drag(qh, &event.surface);
                }
                log::info!(
                    "toolbar press: drag_start={}, surface={}, seat={:?}, inline_active={}",
                    drag,
                    surface_id(&event.surface),
                    self.current_seat_id(),
                    self.inline_toolbars_active()
                );
                self.set_toolbar_dragging(drag);
                self.handle_toolbar_event(toolbar_event);
                self.toolbar.mark_dirty();
                self.input_state.needs_redraw = true;
                self.refresh_keyboard_interactivity();
            }
            return;
        } else if self.pointer_over_toolbar() {
            self.set_toolbar_dragging(false);
            return;
        }
        debug!(
            "Button {} pressed at ({}, {})",
            button, event.position.0, event.position.1
        );
        if self.zoom.active && button == BTN_MIDDLE && !self.zoom.locked {
            self.zoom.start_pan(event.position.0, event.position.1);
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            return;
        }

        let mb = match button {
            BTN_LEFT => MouseButton::Left,
            BTN_MIDDLE => MouseButton::Middle,
            BTN_RIGHT => MouseButton::Right,
            _ => return,
        };

        let (wx, wy) = self.zoomed_world_coords(event.position.0, event.position.1);
        self.input_state.on_mouse_press(mb, wx, wy);
        self.input_state.needs_redraw = true;
    }
}
