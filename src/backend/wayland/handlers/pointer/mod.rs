// Feeds pointer events (motion/buttons/scroll) into the drawing state to keep the canvas reactive.
use log::debug;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use crate::backend::wayland::state::{debug_toolbar_drag_logging_enabled, surface_id};
use crate::input::state::RegionInputSource;

use super::super::state::WaylandState;
use super::route::{InputSurface, RoutedInput};

mod axis;
mod cursor;
mod enter_leave;
mod motion;
mod press;
mod release;

impl PointerHandler for WaylandState {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let routed = self.route_input(&event.surface, event.position);
            if debug_toolbar_drag_logging_enabled() {
                debug!(
                    "pointer {:?}: seat={:?}, surface={}, on_toolbar={}, inline_active={}, pos=({:.1}, {:.1}), drag_active={}, toolbar_dragging={}, pointer_over_toolbar={}",
                    event.kind,
                    self.focus.current_seat_id(),
                    surface_id(&event.surface),
                    routed.surface == InputSurface::Toolbar,
                    routed.inline_toolbars,
                    event.position.0,
                    event.position.1,
                    self.toolbar_drag.is_moving(),
                    self.toolbar_drag.item_dragging(),
                    self.toolbar_chrome.pointer_over_toolbar()
                );
            }
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.handle_pointer_enter(conn, event, routed);
                }
                PointerEventKind::Leave { .. } => {
                    self.handle_pointer_leave(event, routed);
                }
                PointerEventKind::Motion { .. } => {
                    self.handle_pointer_motion(conn, event, routed);
                }
                PointerEventKind::Press { button, serial, .. } => {
                    self.focus.note_activation_serial(serial);
                    let modal_before = self.input_state.screen_modal_is_active();
                    self.handle_pointer_press(conn, qh, event, routed, button);
                    self.refresh_screen_modal_cursor(modal_before, routed, conn);
                }
                PointerEventKind::Release { button, .. } => {
                    let modal_before = self.input_state.screen_modal_is_active();
                    self.handle_pointer_release(event, routed, button);
                    self.refresh_screen_modal_cursor(modal_before, routed, conn);
                }
                PointerEventKind::Axis {
                    vertical, source, ..
                } => {
                    self.handle_pointer_axis(event, routed, vertical, source);
                }
            }
        }
    }
}
