use smithay_client_toolkit::seat::pointer::{
    BTN_LEFT, PointerEvent, PointerEventKind, PointerHandler,
};
use smithay_client_toolkit::shell::WaylandSurface;
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use super::super::AboutWindowState;

impl PointerHandler for AboutWindowState {
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if &event.surface != self.window.wl_surface() {
                continue;
            }
            match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.update_hover(event.position);
                    self.update_cursor(conn);
                }
                PointerEventKind::Leave { .. } => {
                    self.set_hover(None);
                    self.update_cursor(conn);
                }
                PointerEventKind::Press { button, .. } => {
                    if button == BTN_LEFT
                        && let Some(element) = self.element_at(event.position)
                    {
                        self.focus_element(element);
                        self.activate(element);
                    }
                }
                _ => {}
            }
        }
    }
}
