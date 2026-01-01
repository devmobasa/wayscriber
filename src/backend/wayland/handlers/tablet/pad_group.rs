use log::{debug, warn};
use std::sync::Arc;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2, zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2,
    zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2,
};

use super::IgnoredObjectData;
use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTabletPadGroupV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletPadGroupV2,
        event: <ZwpTabletPadGroupV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_group_v2::Event;
        match event {
            Event::Buttons { buttons } => {
                debug!("Tablet pad group buttons: {:?}", buttons);
            }
            Event::Ring { ring } => {
                debug!("Tablet pad ring announced: {:?}", ring.id());
                state.tablet_pad_rings.push(ring);
            }
            Event::Strip { strip } => {
                debug!("Tablet pad strip announced: {:?}", strip.id());
                state.tablet_pad_strips.push(strip);
            }
            Event::Modes { modes } => {
                debug!("Tablet pad group modes: {}", modes);
            }
            Event::Done => {
                debug!("Tablet pad group description complete");
            }
            Event::ModeSwitch { time, serial, mode } => {
                debug!(
                    "Tablet pad group mode switch -> mode {} (serial {}, time {})",
                    mode, serial, time
                );
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_group_v2::{
            EVT_RING_OPCODE, EVT_STRIP_OPCODE,
        };
        match opcode {
            EVT_RING_OPCODE => qhandle.make_data::<ZwpTabletPadRingV2, _>(()),
            EVT_STRIP_OPCODE => qhandle.make_data::<ZwpTabletPadStripV2, _>(()),
            _ => {
                warn!("Ignoring unknown tablet pad group child opcode {}", opcode);
                Arc::new(IgnoredObjectData)
            }
        }
    }
}
