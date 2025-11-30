// Manages seat capabilities (keyboard/pointer availability) and requests the matching devices.
use log::{debug, info, warn};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState, pointer::ThemeSpec};
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
            self.set_current_seat(Some(seat.clone()));
            if self.seat_state.get_keyboard(qh, &seat, None).is_ok() {
                debug!("Keyboard initialized");
            }
        }

        if capability == Capability::Pointer {
            info!("Pointer capability available");
            match self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.compositor_state.create_surface(qh),
                ThemeSpec::default(),
            ) {
                Ok(pointer) => {
                    debug!("Pointer initialized with theme");
                    self.themed_pointer = Some(pointer);
                }
                Err(err) => {
                    warn!("Pointer initialized without theme: {}", err);
                    if self.seat_state.get_pointer(qh, &seat).is_ok() {
                        debug!("Pointer initialized without theme fallback");
                    }
                }
            }
        }

        #[cfg(tablet)]
        if let Some(manager) = &self.tablet_manager {
            if self.tablet_seats.is_empty() {
                let tseat = manager.get_tablet_seat(&seat, qh, ());
                self.tablet_seats.push(tseat);
                info!("Tablet seat initialized for seat");
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
            self.themed_pointer = None;
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
        debug!("Seat removed");
    }
}
