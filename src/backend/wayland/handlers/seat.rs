// Manages seat capabilities (keyboard/pointer availability) and requests the matching devices.
use log::{debug, info, warn};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState, pointer::ThemeSpec};
use wayland_client::{Connection, QueueHandle, protocol::wl_seat};

use super::super::state::WaylandState;
use crate::input::RegionInputSource;

impl SeatHandler for WaylandState {
    fn seat_state(&mut self) -> &mut SeatState {
        self.protocol.seat_mut()
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
        match capability {
            Capability::Keyboard => {
                info!("Keyboard capability available");
                self.focus.set_current_seat(Some(seat.clone()));
                if self
                    .protocol
                    .seat_mut()
                    .get_keyboard(qh, &seat, None)
                    .is_ok()
                {
                    debug!("Keyboard initialized");
                }
                // IME: create the single supported text-input object alongside the
                // first physical keyboard seat. Driven by enable()/disable()
                // reconcile; see the explicit single-seat scope in text_input.rs.
                if self.text_input.attach_if_absent(&seat, qh) {
                    debug!("text-input-v3 object created for seat");
                }
            }
            Capability::Pointer => {
                info!("Pointer capability available");
                let shm = self.protocol.shm().wl_shm().clone();
                let cursor_surface = self.protocol.compositor().create_surface(qh);
                match self.protocol.seat_mut().get_pointer_with_theme(
                    qh,
                    &seat,
                    &shm,
                    cursor_surface,
                    ThemeSpec::default(),
                ) {
                    Ok(pointer) => {
                        debug!("Pointer initialized with theme");
                        self.pointer.attach_pointer(pointer);
                    }
                    Err(err) => {
                        warn!("Pointer initialized without theme: {}", err);
                        if self.protocol.seat_mut().get_pointer(qh, &seat).is_ok() {
                            debug!("Pointer initialized without theme fallback");
                        }
                    }
                }
            }
            Capability::Touch => {
                info!("Touch capability available");
                match self.protocol.seat_mut().get_touch(qh, &seat) {
                    Ok(touch) => {
                        debug!("Touch initialized");
                        self.pointer.attach_touch(touch);
                    }
                    Err(err) => {
                        warn!("Touch initialization failed: {}", err);
                    }
                }
            }
            _ => {}
        }

        #[cfg(feature = "tablet-input")]
        if let Some(manager) = &self.tablet.manager
            && self.tablet.seats.is_empty()
        {
            let tseat = manager.get_tablet_seat(&seat, qh, ());
            self.tablet.seats.push(tseat);
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
            self.cancel_region_selection_from(RegionInputSource::Pointer);
            self.pointer.detach_pointer();
        }
        if capability == Capability::Touch {
            info!("Touch capability removed");
            self.pointer.detach_touch();
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
        if !self.text_input.detach_if_owned(removed_seat) {
            return;
        }
        self.input_state.ime_clear_with(self.render.text_measurer());
        self.input_state.take_text_input_cursor_rect_dirty();
        self.input_state.take_text_input_external_change_dirty();

        let fallback = self.protocol.seat().seats().find(|seat| {
            seat != removed_seat
                && self
                    .protocol
                    .seat()
                    .info(seat)
                    .is_some_and(|info| info.has_keyboard)
        });
        if let Some(seat) = fallback
            && self.text_input.attach_if_absent(&seat, qh)
        {
            debug!("text-input-v3 object failed over to another keyboard seat");
        }
    }
}
