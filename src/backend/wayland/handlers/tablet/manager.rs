use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_manager_v2::ZwpTabletManagerV2;

use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTabletManagerV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletManagerV2,
        _event: <ZwpTabletManagerV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // No events
    }
}
