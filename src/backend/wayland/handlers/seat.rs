// Manages seat capabilities (keyboard/pointer availability) and requests the matching devices.
use log::{debug, info, warn};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState, pointer::ThemeSpec};
use wayland_client::{
    Connection, Proxy, QueueHandle,
    protocol::{wl_keyboard, wl_seat, wl_touch},
};

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
                self.attach_seat_keyboard(qh, &seat);
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
                self.attach_seat_touch(qh, &seat);
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
            self.release_seat_keyboard(&seat);
            self.remove_owned_text_input(&seat, qh);
        }
        if capability == Capability::Pointer {
            info!("Pointer capability removed");
            self.cancel_region_selection_from(RegionInputSource::Pointer);
            self.pointer.detach_pointer();
        }
        if capability == Capability::Touch {
            info!("Touch capability removed");
            self.release_seat_touch(&seat);
            self.cancel_active_touch_sequence();
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        // A seat can vanish without first withdrawing its capabilities.
        self.release_seat_keyboard(&seat);
        if self.release_seat_touch(&seat) {
            self.cancel_active_touch_sequence();
        }
        self.remove_owned_text_input(&seat, qh);
        debug!("Seat removed");
    }
}

/// Whether a seat device of this version has a `release` request. Before
/// version 3, `wl_keyboard` and `wl_touch` have no destructor, so dropping
/// the proxy is all a client can do.
fn seat_device_has_release(version: u32) -> bool {
    version >= 3
}

fn release_keyboard(keyboard: wl_keyboard::WlKeyboard) {
    if seat_device_has_release(keyboard.version()) {
        keyboard.release();
    }
}

fn release_touch(touch: wl_touch::WlTouch) {
    if seat_device_has_release(touch.version()) {
        touch.release();
    }
}

impl WaylandState {
    /// Bind `seat`'s keyboard and keep it, so it can be released later.
    fn attach_seat_keyboard(&mut self, qh: &QueueHandle<Self>, seat: &wl_seat::WlSeat) {
        match self.protocol.seat_mut().get_keyboard(qh, seat, None) {
            Ok(keyboard) => {
                debug!("Keyboard initialized");
                if let Some(replaced) = self.focus.attach_keyboard(seat.clone(), keyboard) {
                    release_keyboard(replaced);
                }
            }
            Err(err) => warn!("Keyboard initialization failed: {}", err),
        }
    }

    fn release_seat_keyboard(&mut self, seat: &wl_seat::WlSeat) {
        if let Some(keyboard) = self.focus.detach_keyboard(seat) {
            release_keyboard(keyboard);
        }
    }

    /// Bind `seat`'s touch device and keep it, so it can be released later.
    fn attach_seat_touch(&mut self, qh: &QueueHandle<Self>, seat: &wl_seat::WlSeat) {
        match self.protocol.seat_mut().get_touch(qh, seat) {
            Ok(touch) => {
                debug!("Touch initialized");
                if let Some(replaced) = self.pointer.attach_touch(seat.clone(), touch) {
                    release_touch(replaced);
                }
            }
            Err(err) => warn!("Touch initialization failed: {}", err),
        }
    }

    /// Release `seat`'s touch device; whether it had one.
    fn release_seat_touch(&mut self, seat: &wl_seat::WlSeat) -> bool {
        let Some(touch) = self.pointer.detach_touch(seat) else {
            return false;
        };

        release_touch(touch);
        true
    }

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

#[cfg(test)]
mod tests {
    use super::seat_device_has_release;

    #[test]
    fn only_version_three_devices_and_later_are_released() {
        assert!(!seat_device_has_release(1));
        assert!(!seat_device_has_release(2));
        assert!(seat_device_has_release(3));
        assert!(seat_device_has_release(9));
    }
}
