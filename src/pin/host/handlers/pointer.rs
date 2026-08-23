use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerHandler};
use wayland_client::{Connection, QueueHandle, protocol::wl_pointer};

use super::super::PinHost;

impl PointerHandler for PinHost {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        self.handle_pointer_frame(pointer, events, qh);
    }
}
