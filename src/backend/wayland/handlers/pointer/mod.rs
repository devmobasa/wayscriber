// Feeds pointer events (motion/buttons/scroll) into the drawing state to keep the canvas reactive.
use log::debug;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use crate::backend::wayland::state::{debug_toolbar_drag_logging_enabled, surface_id};

use super::super::state::WaylandState;

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
            let on_toolbar = self.toolbar.is_toolbar_surface(&event.surface);
            let inline_active = self.inline_toolbars_active() && self.toolbar.is_visible();
            if debug_toolbar_drag_logging_enabled() {
                debug!(
                    "pointer {:?}: seat={:?}, surface={}, on_toolbar={}, inline_active={}, pos=({:.1}, {:.1}), drag_active={}, toolbar_dragging={}, pointer_over_toolbar={}",
                    event.kind,
                    self.current_seat_id(),
                    surface_id(&event.surface),
                    on_toolbar,
                    inline_active,
                    event.position.0,
                    event.position.1,
                    self.is_move_dragging(),
                    self.toolbar_dragging(),
                    self.pointer_over_toolbar()
                );
            }
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.handle_pointer_enter(conn, event, on_toolbar, inline_active);
                }
                PointerEventKind::Leave { .. } => {
                    self.handle_pointer_leave(event, on_toolbar, inline_active);
                }
                PointerEventKind::Motion { .. } => {
                    self.handle_pointer_motion(conn, event, on_toolbar, inline_active);
                }
                PointerEventKind::Press { button, .. } => {
                    self.handle_pointer_press(conn, qh, event, on_toolbar, inline_active, button);
                }
                PointerEventKind::Release { button, .. } => {
                    self.handle_pointer_release(event, on_toolbar, inline_active, button);
                }
                PointerEventKind::Axis { vertical, .. } => {
                    self.handle_pointer_axis(event, on_toolbar, vertical);
                }
            }
        }
    }
}
