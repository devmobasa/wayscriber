use log::debug;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2;

use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTabletPadStripV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletPadStripV2,
        event: <ZwpTabletPadStripV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_strip_v2::Event;
        match event {
            Event::Source { source } => {
                debug!("Tablet pad strip source: {:?}", source);
            }
            Event::Position { position } => {
                debug!("Tablet pad strip position: {}", position);
            }
            Event::Stop => {
                debug!("Tablet pad strip interaction stopped");
            }
            Event::Frame { time } => {
                debug!("Tablet pad strip frame @ {}", time);
            }
            _ => {}
        }
    }
}
