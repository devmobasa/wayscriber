use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_buffer};

use super::super::PinHost;

impl Dispatch<wl_buffer::WlBuffer, ()> for PinHost {
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
