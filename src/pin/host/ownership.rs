//! Single-owner arbitration across pointer, touch, and tablet devices.

use wayland_client::{Proxy, backend::ObjectId};

use super::PinHost;
use crate::pin::PinId;
use crate::pin::surface::{InputOwner, Interaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerDevice {
    Pointer,
    Touch,
    #[cfg(feature = "tablet-input")]
    Stylus,
}

fn same_seat<T: PartialEq>(left: &T, right: &T) -> bool {
    left == right
}

fn lock_belongs_to<T: PartialEq>(locked: Option<T>, pin: T) -> bool {
    locked == Some(pin)
}

fn owner_device(owner: &InputOwner) -> OwnerDevice {
    match owner {
        InputOwner::Pointer { .. } => OwnerDevice::Pointer,
        InputOwner::Touch { .. } => OwnerDevice::Touch,
        #[cfg(feature = "tablet-input")]
        InputOwner::Stylus { .. } => OwnerDevice::Stylus,
    }
}

fn owner_is_on_seat(
    owner: &InputOwner,
    seat: &ObjectId,
    _stylus_on_seat: impl Fn(&ObjectId) -> bool,
) -> bool {
    match owner {
        InputOwner::Pointer {
            seat: owner_seat, ..
        } => same_seat(owner_seat, seat),
        InputOwner::Touch {
            seat: owner_seat, ..
        } => same_seat(owner_seat, seat),
        #[cfg(feature = "tablet-input")]
        InputOwner::Stylus { tool } => _stylus_on_seat(tool),
    }
}

fn cancel_for_new_press(
    target_pin: bool,
    owner_on_new_seat: bool,
    new_pointer: bool,
    old_device: Option<OwnerDevice>,
) -> bool {
    target_pin || owner_on_new_seat || (new_pointer && old_device == Some(OwnerDevice::Pointer))
}

impl PinHost {
    pub(super) fn unlock_pointer_for(&mut self, pin_id: PinId) {
        if lock_belongs_to(self.locked_pin, pin_id) {
            self.unlock_pointer();
        }
    }

    pub(super) fn cancel_owner_for_seat(
        &mut self,
        seat: &wayland_client::protocol::wl_seat::WlSeat,
    ) {
        self.cancel_owners(None, &seat.id(), false);
    }

    pub(super) fn arbitrate_new_owner(&mut self, target_pin: PinId, seat_id: &ObjectId) {
        self.cancel_owners(Some(target_pin), seat_id, false);
    }

    pub(super) fn arbitrate_new_pointer_owner(&mut self, target_pin: PinId, seat_id: &ObjectId) {
        self.cancel_owners(Some(target_pin), seat_id, true);
    }

    fn cancel_owners(&mut self, target_pin: Option<PinId>, seat_id: &ObjectId, new_pointer: bool) {
        let mut cancelled_pins = Vec::new();
        #[cfg(feature = "tablet-input")]
        let tablet_tool_seats = &self.tablet_tool_seats;
        for (pin_id, pin) in &mut self.pins {
            let (device, owner_on_new_seat) = match &pin.interaction {
                Interaction::PressedControl { owner, .. } | Interaction::Dragging { owner, .. } => {
                    (
                        Some(owner_device(owner)),
                        owner_is_on_seat(owner, seat_id, |tool| {
                            #[cfg(feature = "tablet-input")]
                            {
                                tablet_tool_seats.get(tool) == Some(seat_id)
                            }
                            #[cfg(not(feature = "tablet-input"))]
                            {
                                let _ = tool;
                                false
                            }
                        }),
                    )
                }
                Interaction::Idle => (None, false),
            };
            if cancel_for_new_press(
                target_pin == Some(*pin_id),
                owner_on_new_seat,
                new_pointer,
                device,
            ) && !matches!(pin.interaction, Interaction::Idle)
            {
                cancelled_pins.push(*pin_id);
                pin.cancel_interaction();
            }
        }
        self.active_touches.retain(|(touch, _), active| {
            self.touches
                .get(touch)
                .is_some_and(|(_, owner)| owner.id() != *seat_id)
                && !cancelled_pins.contains(&active.pin_id)
        });
        #[cfg(feature = "tablet-input")]
        for (tool, state) in &mut self.stylus_tools {
            if self.tablet_tool_seats.get(tool) == Some(seat_id)
                || state
                    .pin_id
                    .is_some_and(|pin| cancelled_pins.contains(&pin))
            {
                state.tip_down = false;
                state.pending_position = None;
            }
        }
        if self
            .locked_pin
            .is_some_and(|pin| cancelled_pins.contains(&pin))
        {
            self.unlock_pointer();
        }
    }
}

#[cfg(test)]
mod tests {
    use smithay_client_toolkit::seat::pointer::BTN_LEFT;

    use super::*;

    #[test]
    fn cross_device_owner_matching_covers_pointer_touch_and_stylus() {
        let seat = ObjectId::null();
        let pointer = InputOwner::Pointer {
            seat: seat.clone(),
            button: BTN_LEFT,
        };
        let touch = InputOwner::Touch {
            seat: seat.clone(),
            id: 7,
        };
        assert_eq!(owner_device(&pointer), OwnerDevice::Pointer);
        assert_eq!(owner_device(&touch), OwnerDevice::Touch);
        assert!(owner_is_on_seat(&pointer, &seat, |_| false));
        assert!(owner_is_on_seat(&touch, &seat, |_| false));
        #[cfg(feature = "tablet-input")]
        {
            let stylus = InputOwner::Stylus {
                tool: ObjectId::null(),
            };
            assert_eq!(owner_device(&stylus), OwnerDevice::Stylus);
            assert!(owner_is_on_seat(&stylus, &seat, |_| true));
        }
    }

    #[test]
    fn target_pin_and_seat_arbitration_cover_cross_device_and_multi_seat() {
        assert!(cancel_for_new_press(true, false, false, None));
        assert!(cancel_for_new_press(
            false,
            true,
            false,
            Some(OwnerDevice::Touch)
        ));
        assert!(!cancel_for_new_press(
            false,
            false,
            false,
            Some(OwnerDevice::Pointer)
        ));
    }

    #[test]
    fn second_seat_pointer_press_cancels_the_global_pointer_lock_owner() {
        let old_pointer = InputOwner::Pointer {
            seat: ObjectId::null(),
            button: BTN_LEFT,
        };
        assert!(!same_seat(&1_u64, &2_u64));
        assert!(cancel_for_new_press(
            false,
            false,
            true,
            Some(owner_device(&old_pointer))
        ));
        assert!(!cancel_for_new_press(
            false,
            false,
            true,
            Some(OwnerDevice::Touch)
        ));
    }

    #[test]
    fn scoped_unlock_never_drops_another_pins_constraint() {
        assert!(!lock_belongs_to(Some(1_u64), 2));
        assert!(lock_belongs_to(Some(1_u64), 1));
    }

    #[test]
    #[cfg(feature = "tablet-input")]
    fn removed_tablet_seat_correlates_its_stylus_owner() {
        let seat = ObjectId::null();
        let stylus = InputOwner::Stylus {
            tool: ObjectId::null(),
        };
        assert!(owner_is_on_seat(&stylus, &seat, |_| true));
    }
}
