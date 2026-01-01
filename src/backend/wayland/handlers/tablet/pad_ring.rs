use log::debug;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2;

use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTabletPadRingV2, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletPadRingV2,
        event: <ZwpTabletPadRingV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_ring_v2::Event;
        match event {
            Event::Source { source } => {
                debug!("Tablet pad ring source: {:?}", source);
            }
            Event::Angle { degrees } => {
                debug!("Tablet pad ring angle: {:?}", degrees);
            }
            Event::Stop => {
                debug!("Tablet pad ring interaction stopped");
            }
            Event::Frame { time } => {
                debug!("Tablet pad ring frame @ {}", time);
            }
            _ => {}
        }
    }
}
