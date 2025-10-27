// Listens for wl_buffer release events so SlotPool buffers can re-enter circulation.
use log::debug;
use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_buffer};

use super::super::state::WaylandState;

impl Dispatch<wl_buffer::WlBuffer, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            debug!("Buffer released by compositor");
        }
    }
}
