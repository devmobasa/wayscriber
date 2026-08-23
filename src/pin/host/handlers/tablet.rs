//! Tablet-v2 stylus routing for pin surfaces.

use std::os::fd::OwnedFd;
use std::sync::Arc;

use wayland_client::backend::protocol::Message;
use wayland_client::backend::{Backend, ObjectData, ObjectId};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2::ZwpTabletManagerV2,
    zwp_tablet_pad_v2::ZwpTabletPadV2,
    zwp_tablet_seat_v2::{self, ZwpTabletSeatV2},
    zwp_tablet_tool_v2::{self, ZwpTabletToolV2},
    zwp_tablet_v2::ZwpTabletV2,
};

use super::super::{PinHost, StylusToolState};
use crate::pin::surface::{InputOwner, Interaction, ReleaseAction};

#[derive(Debug)]
struct IgnoredObjectData;

impl ObjectData for IgnoredObjectData {
    fn event(
        self: Arc<Self>,
        _backend: &Backend,
        _msg: Message<ObjectId, OwnedFd>,
    ) -> Option<Arc<dyn ObjectData>> {
        None
    }

    fn destroyed(&self, _object_id: ObjectId) {}
}

impl Dispatch<ZwpTabletManagerV2, ()> for PinHost {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTabletManagerV2,
        _event: <ZwpTabletManagerV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTabletSeatV2, ()> for PinHost {
    fn event(
        state: &mut Self,
        tablet_seat: &ZwpTabletSeatV2,
        event: <ZwpTabletSeatV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let zwp_tablet_seat_v2::Event::ToolAdded { id } = event {
            if let Some(seat) = state
                .tablet_seats
                .iter()
                .find_map(|(seat, proxy)| (proxy.id() == tablet_seat.id()).then_some(seat.clone()))
            {
                state.tablet_tool_seats.insert(id.id(), seat);
            }
            state
                .stylus_tools
                .insert(id.id(), StylusToolState::default());
            state.tablet_tools.insert(id.id(), id);
        }
    }

    fn event_created_child(opcode: u16, qhandle: &QueueHandle<Self>) -> Arc<dyn ObjectData> {
        match opcode {
            zwp_tablet_seat_v2::EVT_TOOL_ADDED_OPCODE => {
                qhandle.make_data::<ZwpTabletToolV2, _>(())
            }
            zwp_tablet_seat_v2::EVT_TABLET_ADDED_OPCODE => Arc::new(IgnoredObjectData),
            zwp_tablet_seat_v2::EVT_PAD_ADDED_OPCODE => Arc::new(IgnoredObjectData),
            _ => Arc::new(IgnoredObjectData),
        }
    }
}

impl Dispatch<ZwpTabletToolV2, ()> for PinHost {
    fn event(
        state: &mut Self,
        tool: &ZwpTabletToolV2,
        event: <ZwpTabletToolV2 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let tool_id = tool.id();
        match event {
            zwp_tablet_tool_v2::Event::ProximityIn { surface, .. } => {
                let pin_id = state.by_wl_surface.get(&surface.id()).copied();
                let tool_state = state.stylus_tools.entry(tool_id).or_default();
                tool_state.pin_id = pin_id;
                tool_state.tip_down = false;
                tool_state.pending_position = None;
            }
            zwp_tablet_tool_v2::Event::Motion { x, y } => {
                if let Some(tool_state) = state.stylus_tools.get_mut(&tool_id) {
                    tool_state.pending_position = Some((x, y));
                }
            }
            zwp_tablet_tool_v2::Event::Down { .. } => {
                state.begin_stylus_press(tool_id);
            }
            zwp_tablet_tool_v2::Event::Up => {
                state.finish_stylus_press(tool_id);
            }
            zwp_tablet_tool_v2::Event::Frame { .. } => {
                state.commit_stylus_frame(tool_id, qh);
            }
            zwp_tablet_tool_v2::Event::ProximityOut => {
                state.cancel_stylus(tool_id);
            }
            zwp_tablet_tool_v2::Event::Removed => {
                state.cancel_stylus(tool_id.clone());
                state.stylus_tools.remove(&tool_id);
                state.tablet_tools.remove(&tool_id);
                state.tablet_tool_seats.remove(&tool_id);
            }
            _ => {}
        }
    }
}

impl PinHost {
    fn begin_stylus_press(&mut self, tool_id: ObjectId) {
        let Some(seat_id) = self.tablet_tool_seats.get(&tool_id).cloned() else {
            return;
        };
        let Some(tool_state) = self.stylus_tools.get_mut(&tool_id) else {
            return;
        };
        if let Some(position) = tool_state.pending_position.take() {
            tool_state.position = position;
        }
        let Some(pin_id) = tool_state.pin_id else {
            return;
        };
        let position = tool_state.position;
        self.arbitrate_new_owner(pin_id, &seat_id);
        let Some(tool_state) = self.stylus_tools.get_mut(&tool_id) else {
            return;
        };
        let owner = InputOwner::Stylus {
            tool: tool_id.clone(),
        };
        if let Some(pin) = self.pins.get_mut(&pin_id) {
            pin.press(owner, position);
            tool_state.tip_down = true;
        }
    }

    fn commit_stylus_frame(&mut self, tool_id: ObjectId, qh: &QueueHandle<Self>) {
        let Some(tool_state) = self.stylus_tools.get_mut(&tool_id) else {
            return;
        };
        let Some(position) = tool_state.pending_position.take() else {
            return;
        };
        tool_state.position = position;
        let Some(pin_id) = tool_state.pin_id else {
            return;
        };
        if !tool_state.tip_down {
            if let Some(pin) = self.pins.get_mut(&pin_id) {
                pin.update_hover(Some(position));
            }
            return;
        }
        let owner = InputOwner::Stylus {
            tool: tool_id.clone(),
        };
        let candidate = self
            .pins
            .get_mut(&pin_id)
            .and_then(|pin| pin.fallback_drag_origin(&owner, position, pin.shell.committed_origin));
        if let Some(candidate) = candidate {
            self.apply_drag_origin(pin_id, candidate, qh);
        }
    }

    fn finish_stylus_press(&mut self, tool_id: ObjectId) {
        let Some(tool_state) = self.stylus_tools.get_mut(&tool_id) else {
            return;
        };
        if let Some(position) = tool_state.pending_position.take() {
            tool_state.position = position;
        }
        tool_state.tip_down = false;
        let Some(pin_id) = tool_state.pin_id else {
            return;
        };
        let owner = InputOwner::Stylus { tool: tool_id };
        let action = self
            .pins
            .get_mut(&pin_id)
            .map_or(ReleaseAction::None, |pin| {
                pin.release(&owner, tool_state.position)
            });
        self.apply_release(pin_id, action);
    }

    fn cancel_stylus(&mut self, tool_id: ObjectId) {
        let pin_id = self.stylus_tools.get(&tool_id).and_then(|tool| tool.pin_id);
        if let Some(pin_id) = pin_id
            && let Some(pin) = self.pins.get_mut(&pin_id)
        {
            let owned = matches!(
                &pin.interaction,
                Interaction::PressedControl {
                    owner: InputOwner::Stylus { tool },
                    ..
                } | Interaction::Dragging {
                    owner: InputOwner::Stylus { tool },
                    ..
                } if *tool == tool_id
            );
            if owned {
                pin.cancel_interaction();
            }
            pin.update_hover(None);
        }
        if let Some(tool_state) = self.stylus_tools.get_mut(&tool_id) {
            tool_state.pin_id = None;
            tool_state.tip_down = false;
            tool_state.pending_position = None;
        }
    }
}

// Child objects deliberately ignored by the pin host still need type names in
// this module so the protocol's generated event enum remains fully resolved.
const _: Option<(ZwpTabletV2, ZwpTabletPadV2)> = None;
