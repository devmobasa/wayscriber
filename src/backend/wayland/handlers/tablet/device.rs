use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_v2::ZwpTabletV2;

use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTabletV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletV2,
        _event: <ZwpTabletV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Descriptive events are ignored.
    }
}
