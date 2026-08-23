use smithay_client_toolkit::seat::pointer_constraints::PointerConstraintsHandler;
use wayland_client::{
    Connection, Proxy, QueueHandle,
    protocol::{wl_pointer, wl_surface},
};
use wayland_protocols::wp::pointer_constraints::zv1::client::{
    zwp_confined_pointer_v1::ZwpConfinedPointerV1, zwp_locked_pointer_v1::ZwpLockedPointerV1,
};

use super::super::{PinHost, proxy_identity};

impl PointerConstraintsHandler for PinHost {
    fn confined(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _confined_pointer: &ZwpConfinedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
    }

    fn unconfined(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _confined_pointer: &ZwpConfinedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
    }

    fn locked(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        locked_pointer: &ZwpLockedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
        // The proxy is installed synchronously when the lock is requested. A
        // delayed event from a destroyed earlier proxy must not replace it.
        if !proxy_identity::is_current(
            self.locked_pointer.as_ref().map(Proxy::id),
            locked_pointer.id(),
        ) {
            log::trace!("Ignoring stale pin pointer-constraint locked event");
        }
    }

    fn unlocked(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        locked_pointer: &ZwpLockedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
        if proxy_identity::is_current(
            self.locked_pointer.as_ref().map(Proxy::id),
            locked_pointer.id(),
        ) {
            self.unlock_pointer();
        } else {
            log::trace!("Ignoring stale pin pointer-constraint unlocked event");
        }
    }
}
