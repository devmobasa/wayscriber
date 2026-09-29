/// The input device each seat handed out, kept so the device can be released
/// when its seat loses the capability or goes away.
///
/// Keyed by seat because the overlay can have more than one: releasing the
/// wrong seat's device would silence that seat, and dropping a proxy without
/// releasing it leaves the object alive on the compositor.
pub(in crate::backend::wayland) struct SeatDevices<S, D> {
    devices: Vec<(S, D)>,
}

impl<S, D> Default for SeatDevices<S, D> {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
        }
    }
}

impl<S: PartialEq, D> SeatDevices<S, D> {
    /// Keep `device` for `seat`, returning the device it replaces, if any.
    pub(in crate::backend::wayland) fn attach(&mut self, seat: S, device: D) -> Option<D> {
        let replaced = self.detach(&seat);

        self.devices.push((seat, device));
        replaced
    }

    /// Stop keeping `seat`'s device and hand it back for release.
    pub(in crate::backend::wayland) fn detach(&mut self, seat: &S) -> Option<D> {
        let index = self.devices.iter().position(|(owner, _)| owner == seat)?;

        Some(self.devices.swap_remove(index).1)
    }
}

#[cfg(test)]
mod tests {
    use super::SeatDevices;

    #[test]
    fn each_seat_keeps_its_own_device() {
        let mut devices = SeatDevices::default();

        assert_eq!(devices.attach(1, "first keyboard"), None);
        assert_eq!(devices.attach(2, "second keyboard"), None);

        assert_eq!(devices.detach(&2), Some("second keyboard"));
        assert_eq!(devices.detach(&1), Some("first keyboard"));
    }

    #[test]
    fn detaching_one_seat_leaves_the_others_alone() {
        let mut devices = SeatDevices::default();
        devices.attach(1, "first");
        devices.attach(2, "second");

        assert_eq!(devices.detach(&1), Some("first"));
        assert_eq!(devices.detach(&1), None);
        assert_eq!(devices.detach(&2), Some("second"));
    }

    #[test]
    fn a_second_device_for_one_seat_hands_back_the_first() {
        let mut devices = SeatDevices::default();
        devices.attach(1, "old");

        assert_eq!(devices.attach(1, "new"), Some("old"));
        assert_eq!(devices.detach(&1), Some("new"));
        assert_eq!(devices.detach(&1), None);
    }

    #[test]
    fn a_seat_that_never_had_a_device_hands_back_nothing() {
        let mut devices: SeatDevices<u32, &str> = SeatDevices::default();

        assert_eq!(devices.detach(&7), None);
    }
}
