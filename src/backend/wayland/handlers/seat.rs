// Manages seat capabilities (keyboard/pointer availability) and requests the matching devices.
use log::{debug, info};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use wayland_client::{Connection, QueueHandle, protocol::wl_seat};

use super::super::state::WaylandState;

impl SeatHandler for WaylandState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        debug!("New seat available");
    }

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            info!("Keyboard capability available");
            if self.seat_state.get_keyboard(qh, &seat, None).is_ok() {
                debug!("Keyboard initialized");
            }
        }

        if capability == Capability::Pointer {
            info!("Pointer capability available");
            if self.seat_state.get_pointer(qh, &seat).is_ok() {
                debug!("Pointer initialized");
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            info!("Keyboard capability removed");
        }
        if capability == Capability::Pointer {
            info!("Pointer capability removed");
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        debug!("Seat removed");
    }
}
