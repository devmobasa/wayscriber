use smithay_client_toolkit::seat::relative_pointer::{RelativeMotionEvent, RelativePointerHandler};
use wayland_client::{Connection, Proxy, QueueHandle, protocol::wl_pointer};
use wayland_protocols::wp::relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1;

use super::super::{PinHost, proxy_identity};

impl RelativePointerHandler for PinHost {
    fn relative_pointer_motion(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        relative_pointer: &ZwpRelativePointerV1,
        pointer: &wl_pointer::WlPointer,
        event: RelativeMotionEvent,
    ) {
        if proxy_identity::is_current(
            self.relative_pointer.as_ref().map(Proxy::id),
            relative_pointer.id(),
        ) {
            self.handle_relative_motion(pointer.id(), event.delta, qh);
        } else {
            log::trace!("Ignoring stale pin relative-pointer motion event");
        }
    }
}
