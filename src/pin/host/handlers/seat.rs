use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use wayland_client::{Connection, Proxy, QueueHandle, protocol::wl_seat};

use super::super::PinHost;

fn matching_owner_ids<K: Clone, S: PartialEq>(
    entries: impl Iterator<Item = (K, S)>,
    target: &S,
) -> Vec<K> {
    entries
        .filter_map(|(id, owner)| (owner == *target).then_some(id))
        .collect()
}

impl SeatHandler for PinHost {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        #[cfg(feature = "tablet-input")]
        if let Some(manager) = &self.tablet_manager {
            self.tablet_seats
                .insert(seat.id(), manager.get_tablet_seat(&seat, qh, ()));
        }
        #[cfg(not(feature = "tablet-input"))]
        let _ = (qh, seat);
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Pointer => match self.seat_state.get_pointer(qh, &seat) {
                Ok(pointer) => {
                    if let Some(manager) = &self.cursor_shape_manager {
                        self.cursor_shape_devices
                            .insert(pointer.id(), manager.get_shape_device(&pointer, qh));
                    }
                    self.pointers.insert(pointer.id(), (pointer, seat));
                }
                Err(error) => log::warn!("Failed to initialize pin pointer: {error}"),
            },
            Capability::Touch => match self.seat_state.get_touch(qh, &seat) {
                Ok(touch) => {
                    self.touches.insert(touch.id(), (touch, seat));
                }
                Err(error) => log::warn!("Failed to initialize pin touch: {error}"),
            },
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            let ids: Vec<_> = self
                .pointers
                .iter()
                .filter_map(|(id, (_, owner))| (owner == &seat).then_some(id.clone()))
                .collect();
            for id in ids {
                if let Some((pointer, _)) = self.pointers.get(&id).cloned() {
                    self.cancel_owner_for_pointer(&pointer);
                }
                self.pointers.remove(&id);
                self.cursor_shape_devices.remove(&id);
                self.cursor_serials.remove(&id);
            }
        }
        if capability == Capability::Touch {
            self.cancel_owner_for_seat(&seat);
            self.touches.retain(|_, (_, owner)| owner != &seat);
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.cancel_owner_for_seat(&seat);
        let pointer_ids = matching_owner_ids(
            self.pointers
                .iter()
                .map(|(id, (_, owner))| (id.clone(), owner.clone())),
            &seat,
        );
        for id in pointer_ids {
            self.pointers.remove(&id);
            self.cursor_shape_devices.remove(&id);
            self.cursor_serials.remove(&id);
        }
        #[cfg(feature = "tablet-input")]
        {
            self.tablet_seats.remove(&seat.id());
            self.tablet_tool_seats
                .retain(|_, tool_seat| *tool_seat != seat.id());
        }
        self.touches.retain(|_, (_, owner)| owner != &seat);
    }
}

#[cfg(test)]
mod tests {
    use super::matching_owner_ids;

    #[test]
    fn seat_removal_correlates_all_pointer_side_tables() {
        let entries = [(1_u64, "seat-a"), (2, "seat-b"), (3, "seat-a")];
        assert_eq!(
            matching_owner_ids(entries.into_iter(), &"seat-a"),
            vec![1, 3]
        );
    }
}
