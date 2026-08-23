//! Pure host exit eligibility shared by runtime checks and regressions.

use super::PinHost;

impl PinHost {
    pub(super) fn maybe_finish(&mut self) {
        if self.shutdown_armed && self.pins.is_empty() && !self.clipboard.is_active() {
            self.should_exit = true;
        }
    }

    pub(super) fn finish_create_transaction(&mut self) {
        if self.pins.is_empty() {
            self.shutdown_armed = true;
        }
        self.maybe_finish();
    }
}

pub(super) fn exit_shutdown_eligible(
    host_should_exit: bool,
    decoder_active: bool,
    pending_ready: bool,
    active_clients: usize,
    drain_grace_elapsed: bool,
) -> bool {
    host_should_exit
        && !decoder_active
        && !pending_ready
        && active_clients == 0
        && drain_grace_elapsed
}

pub(super) const fn ready_delivery_allowed(wayland_flushed: bool) -> bool {
    wayland_flushed
}

#[cfg(test)]
mod tests {
    use super::{exit_shutdown_eligible, ready_delivery_allowed};

    #[test]
    fn refused_first_client_cannot_drop_second_negotiated_create() {
        assert!(!exit_shutdown_eligible(true, false, false, 1, true));
        assert!(!exit_shutdown_eligible(true, true, false, 0, true));
        assert!(!exit_shutdown_eligible(false, false, false, 0, true));
    }

    #[test]
    fn empty_terminal_host_waits_for_listener_drain_grace() {
        assert!(!exit_shutdown_eligible(true, false, false, 0, false));
        assert!(exit_shutdown_eligible(true, false, false, 0, true));
    }

    #[test]
    fn ready_requires_the_initial_wayland_commit_to_be_flushed() {
        assert!(!ready_delivery_allowed(false));
        assert!(ready_delivery_allowed(true));
    }
}
