use log::debug;
use smithay_client_toolkit::seat::pointer_constraints::PointerConstraintsHandler;
use wayland_client::{
    Connection, QueueHandle,
    protocol::{wl_pointer, wl_surface},
};
use wayland_protocols::wp::pointer_constraints::zv1::client::{
    zwp_confined_pointer_v1::ZwpConfinedPointerV1, zwp_locked_pointer_v1::ZwpLockedPointerV1,
};

use super::super::state::WaylandState;

impl PointerConstraintsHandler for WaylandState {
    fn confined(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _confined_pointer: &ZwpConfinedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
        // Not used; we rely on pointer lock for drags.
    }

    fn unconfined(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _confined_pointer: &ZwpConfinedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
        // Not used.
    }

    fn locked(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        locked_pointer: &ZwpLockedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
        debug!("Pointer lock activated for toolbar drag");
        self.locked_pointer = Some(locked_pointer.clone());
    }

    fn unlocked(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _locked_pointer: &ZwpLockedPointerV1,
        _surface: &wl_surface::WlSurface,
        _pointer: &wl_pointer::WlPointer,
    ) {
        debug!("Pointer lock deactivated for toolbar drag");
        self.unlock_pointer();
    }
}
