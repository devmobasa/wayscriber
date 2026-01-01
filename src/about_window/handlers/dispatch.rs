use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_buffer};

use super::super::AboutWindowState;

impl Dispatch<wl_buffer::WlBuffer, ()> for AboutWindowState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}
