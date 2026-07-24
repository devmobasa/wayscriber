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
            // IME: create the single supported text-input object alongside the
            // first physical keyboard seat. Driven by enable()/disable()
            // reconcile; see the explicit single-seat scope in text_input.rs.
            if self.text_input.is_none()
                && let Some(manager) = &self.text_input_manager
            {
                self.text_input = Some(manager.get_text_input(&seat, qh, ()));
                self.text_input_seat = Some(seat.clone());
                self.text_input_focused = false;
                self.text_input_enabled = false;
                self.text_input_serial = 0;
                self.text_input_cursor_update_pending = false;
                debug!("text-input-v3 object created for seat");
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
                    self.current_pointer_shape = None;
                    self.cursor_hidden = false;
                }
                Err(err) => {
                    warn!("Pointer initialized without theme: {}", err);
                    if self.seat_state.get_pointer(qh, &seat).is_ok() {
                        debug!("Pointer initialized without theme fallback");
                    }
                }
            }
        }

        if capability == Capability::Touch {
            info!("Touch capability available");
            match self.seat_state.get_touch(qh, &seat) {
                Ok(touch) => {
                    debug!("Touch initialized");
                    self.touch = Some(touch);
                }
                Err(err) => {
                    warn!("Touch initialization failed: {}", err);
                }
            }
        }

        #[cfg(feature = "tablet-input")]
        if let Some(manager) = &self.tablet_manager
            && self.tablet_seats.is_empty()
        {
            let tseat = manager.get_tablet_seat(&seat, qh, ());
            self.tablet_seats.push(tseat);
            info!("Tablet seat initialized for seat");
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            info!("Keyboard capability removed");
            self.remove_owned_text_input(&seat, qh);
        }
        if capability == Capability::Pointer {
            info!("Pointer capability removed");
            self.themed_pointer = None;
            self.current_pointer_shape = None;
            self.cursor_hidden = false;
        }
        if capability == Capability::Touch {
            info!("Touch capability removed");
            self.touch = None;
            self.cancel_active_touch_sequence();
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        self.remove_owned_text_input(&seat, qh);
        debug!("Seat removed");
    }
}

impl WaylandState {
    /// Retire the singleton only when its owning seat disappears, then fail
    /// over to another physical-keyboard seat if one is already advertised.
    /// Every new protocol object starts its own commit serial at zero.
    fn remove_owned_text_input(&mut self, removed_seat: &wl_seat::WlSeat, qh: &QueueHandle<Self>) {
        if self.text_input_seat.as_ref() != Some(removed_seat) {
            return;
        }

        if let Some(ti) = self.text_input.take() {
            ti.destroy();
        }
        self.text_input_seat = None;
        self.text_input_focused = false;
        self.text_input_enabled = false;
        self.text_input_serial = 0;
        self.text_input_cursor_update_pending = false;
        self.input_state.ime_clear();

        let fallback = self.seat_state.seats().find(|seat| {
            seat != removed_seat
                && self
                    .seat_state
                    .info(seat)
                    .is_some_and(|info| info.has_keyboard)
        });
        if let (Some(seat), Some(manager)) = (fallback, &self.text_input_manager) {
            self.text_input = Some(manager.get_text_input(&seat, qh, ()));
            self.text_input_seat = Some(seat);
            debug!("text-input-v3 object failed over to another keyboard seat");
        }
    }
}
