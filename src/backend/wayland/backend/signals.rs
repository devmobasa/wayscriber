use std::io;
use std::os::fd::AsRawFd;

use crate::unix_signals::{ShutdownSignal, SignalEvent, SignalEventSource};

pub(super) struct OverlaySignalState<'source> {
    source: &'source mut dyn SignalEventSource,
    exit_requested: bool,
    tray_action_requested: bool,
}

impl<'source> OverlaySignalState<'source> {
    pub(super) fn new(source: &'source mut dyn SignalEventSource) -> Self {
        Self {
            source,
            exit_requested: false,
            tray_action_requested: false,
        }
    }

    pub(super) fn poll_raw_fd(&self) -> io::Result<libc::c_int> {
        Ok(self.source.poll_fd()?.as_raw_fd())
    }

    pub(super) fn drain_events(&mut self) -> io::Result<()> {
        for event in self.source.drain()? {
            match event {
                SignalEvent::ToggleOverlay => {
                    // SIGUSR1 belongs to daemon visibility control. Overlay
                    // processes intentionally consume and ignore it.
                    log::debug!("Overlay received SIGUSR1; ignoring");
                }
                SignalEvent::TrayAction => {
                    log::debug!("Overlay received SIGUSR2 for tray action");
                    self.tray_action_requested = true;
                }
                SignalEvent::Shutdown(signal) => {
                    let name = match signal {
                        ShutdownSignal::Interrupt => "SIGINT",
                        ShutdownSignal::Terminate => "SIGTERM",
                    };
                    log::debug!("Overlay received {name}; scheduling graceful shutdown");
                    self.exit_requested = true;
                }
            }
        }
        Ok(())
    }

    pub(super) fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    pub(super) fn take_tray_action_requested(&mut self) -> bool {
        std::mem::take(&mut self.tray_action_requested)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unix_signals::{FakeSignalSource, ShutdownSignal};

    #[test]
    fn tray_and_shutdown_events_are_owned_by_overlay_state() {
        let mut source = FakeSignalSource::new().expect("fixture creates its signal source");
        source
            .publish(SignalEvent::TrayAction)
            .expect("fixture publishes its tray event");
        source
            .publish(SignalEvent::Shutdown(ShutdownSignal::Terminate))
            .expect("fixture publishes its shutdown event");
        let mut signals = OverlaySignalState::new(&mut source);

        signals
            .drain_events()
            .expect("fixture drains its overlay signal events");

        assert!(signals.take_tray_action_requested());
        assert!(!signals.take_tray_action_requested());
        assert!(signals.exit_requested());
    }

    #[test]
    fn daemon_toggle_signal_is_consumed_without_exiting_overlay() {
        let mut source = FakeSignalSource::new().expect("fixture creates its signal source");
        source
            .publish(SignalEvent::ToggleOverlay)
            .expect("fixture publishes its daemon-only toggle event");
        let mut signals = OverlaySignalState::new(&mut source);

        signals
            .drain_events()
            .expect("fixture drains its ignored overlay signal event");

        assert!(!signals.exit_requested());
        assert!(!signals.take_tray_action_requested());
    }

    #[test]
    fn source_failure_is_reported_to_the_overlay_owner() {
        let mut source = FakeSignalSource::new().expect("fixture creates its signal source");
        source
            .fail_next_drain(io::ErrorKind::BrokenPipe)
            .expect("fixture wakes its failed signal source");
        let mut signals = OverlaySignalState::new(&mut source);

        assert_eq!(
            signals
                .drain_events()
                .expect_err("fixture observes the source failure")
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
