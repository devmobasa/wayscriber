//! Dispatch stubs for the wlr virtual-pointer protocol. Neither interface
//! sends events; the objects exist only so step capture can forward an
//! intercepted click to the application beneath the overlay.

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_manager_v1::{
    self, ZwlrVirtualPointerManagerV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::zwlr_virtual_pointer_v1::{
    self, ZwlrVirtualPointerV1,
};

use super::super::state::WaylandState;

impl Dispatch<ZwlrVirtualPointerManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerManagerV1,
        _event: zwlr_virtual_pointer_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrVirtualPointerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrVirtualPointerV1,
        _event: zwlr_virtual_pointer_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}
