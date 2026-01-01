use log::{debug, info, warn};
use std::sync::Arc;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2, zwp_tablet_pad_v2::ZwpTabletPadV2,
};

use super::IgnoredObjectData;
use crate::backend::wayland::state::WaylandState;

impl Dispatch<ZwpTabletPadV2, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTabletPadV2,
        event: <ZwpTabletPadV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::Event;
        match event {
            Event::Group { pad_group } => {
                debug!("Tablet pad group announced: {:?}", pad_group.id());
                state.tablet_pad_groups.push(pad_group);
            }
            Event::Path { path } => {
                debug!("Tablet pad path: {}", path);
            }
            Event::Buttons { buttons } => {
                debug!("Tablet pad button count: {}", buttons);
            }
            Event::Done => {
                debug!("Tablet pad description complete");
            }
            Event::Button {
                time,
                button,
                state,
            } => {
                debug!(
                    "Tablet pad button event: index {} -> {:?} @ {}",
                    button, state, time
                );
            }
            Event::Enter {
                serial,
                tablet,
                surface,
            } => {
                debug!(
                    "Tablet pad entered surface {:?} (tablet {:?}) serial {}",
                    surface.id(),
                    tablet.id(),
                    serial
                );
            }
            Event::Leave { serial, surface } => {
                debug!(
                    "Tablet pad left surface {:?} serial {}",
                    surface.id(),
                    serial
                );
            }
            Event::Removed => {
                info!("Tablet pad removed");
                state.tablet_pads.clear();
                state.tablet_pad_groups.clear();
                state.tablet_pad_rings.clear();
                state.tablet_pad_strips.clear();
            }
            _ => {}
        }
    }

    fn event_created_child(
        opcode: u16,
        qhandle: &QueueHandle<Self>,
    ) -> Arc<dyn wayland_client::backend::ObjectData> {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_pad_v2::EVT_GROUP_OPCODE;
        match opcode {
            EVT_GROUP_OPCODE => qhandle.make_data::<ZwpTabletPadGroupV2, _>(()),
            _ => {
                warn!("Ignoring unknown tablet pad child opcode {}", opcode);
                Arc::new(IgnoredObjectData)
            }
        }
    }
}
