use log::{info, warn};
use std::sync::Arc;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_pad_v2::ZwpTabletPadV2, zwp_tablet_seat_v2::ZwpTabletSeatV2,
    zwp_tablet_tool_v2::ZwpTabletToolV2, zwp_tablet_v2::ZwpTabletV2,
};

use super::IgnoredObjectData;
use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTabletSeatV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletSeatV2,
        event: <ZwpTabletSeatV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::Event;
        match event {
            Event::TabletAdded { id } => {
                info!("🖊️  TABLET DEVICE DETECTED");
                state.tablets.push(id);
                if !state.tablet_found_logged {
                    state.tablet_found_logged = true;
                    info!("TABLET FOUND - Total devices: {}", state.tablets.len());
                }
            }
            Event::ToolAdded { id } => {
                info!("🖊️  TABLET TOOL DETECTED (pen/stylus)");
                state.tablet_tools.push(id);
                if !state.tablet_found_logged {
                    state.tablet_found_logged = true;
                    info!("TABLET FOUND - Total tools: {}", state.tablet_tools.len());
                }
            }
            Event::PadAdded { id } => {
                info!("🖊️  TABLET PAD DETECTED");
                state.tablet_pads.push(id);
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_seat_v2::{
            EVT_PAD_ADDED_OPCODE, EVT_TABLET_ADDED_OPCODE, EVT_TOOL_ADDED_OPCODE,
        };
        match opcode {
            EVT_TABLET_ADDED_OPCODE => qhandle.make_data::<ZwpTabletV2, _>(()),
            EVT_TOOL_ADDED_OPCODE => qhandle.make_data::<ZwpTabletToolV2, _>(()),
            EVT_PAD_ADDED_OPCODE => qhandle.make_data::<ZwpTabletPadV2, _>(()),
            _ => {
                warn!("Ignoring unknown tablet seat child opcode {}", opcode);
                Arc::new(IgnoredObjectData)
            }
        }
    }
}
