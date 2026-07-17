use std::io;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[cfg(unix)]
use libc::{SIGINT, SIGTERM, SIGUSR1, SIGUSR2};

use super::runtime_wake::RuntimeWakeHandle;

pub(super) struct OverlaySignalState {
    exit_requested: Arc<AtomicBool>,
    tray_action_requested: Arc<AtomicBool>,
    #[cfg(unix)]
    listener: crate::unix_signals::SignalListener,
}

impl OverlaySignalState {
    pub(super) fn exit_requested(&self) -> bool {
        self.exit_requested.load(Ordering::Acquire)
    }

    pub(super) fn take_tray_action_requested(&self) -> bool {
        self.tray_action_requested.swap(false, Ordering::AcqRel)
    }

    pub(super) fn failure(&self) -> Option<String> {
        #[cfg(unix)]
        {
            match self.listener.health() {
                crate::unix_signals::SignalListenerHealth::Failed(failure) => {
                    Some(failure.to_string())
                }
                _ => None,
            }
        }

        #[cfg(not(unix))]
        {
            None
        }
    }

    pub(super) fn stop_and_join(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            let failure = self.failure();
            self.listener.stop_and_join()?;
            match failure {
                Some(failure) => Err(io::Error::other(format!(
                    "overlay signal listener failed before teardown: {failure}"
                ))),
                None => Ok(()),
            }
        }

        #[cfg(not(unix))]
        {
            Ok(())
        }
    }
}

pub(super) fn setup_signal_handlers(
    runtime_wake: RuntimeWakeHandle,
) -> io::Result<OverlaySignalState> {
    let exit_requested = Arc::new(AtomicBool::new(false));
    let tray_action_requested = Arc::new(AtomicBool::new(false));

    #[cfg(unix)]
    {
        let signal_exit_requested = Arc::clone(&exit_requested);
        let signal_tray_action_requested = Arc::clone(&tray_action_requested);
        let listener_wake = runtime_wake;
        let listener = crate::unix_signals::spawn_listener(
            &[SIGTERM, SIGINT, SIGUSR1, SIGUSR2],
            move |signal| match signal {
                SIGUSR1 => {
                    // SIGUSR1 is reserved for daemon toggle; ignore in overlay.
                    log::debug!("Overlay received SIGUSR1; ignoring");
                }
                SIGUSR2 => {
                    log::debug!("Overlay received SIGUSR2 for tray action");
                    signal_tray_action_requested.store(true, Ordering::Release);
                }
                _ => {
                    log::debug!("Overlay received signal {signal}; scheduling graceful shutdown");
                    signal_exit_requested.store(true, Ordering::Release);
                }
            },
            move || {
                if let Err(err) = listener_wake.wake() {
                    log::warn!("Failed to wake overlay after signal publication: {err}");
                }
            },
        )?;

        Ok(OverlaySignalState {
            exit_requested,
            tray_action_requested,
            listener,
        })
    }

    #[cfg(not(unix))]
    {
        let _ = runtime_wake;
        Ok(OverlaySignalState {
            exit_requested,
            tray_action_requested,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::backend::wayland::RuntimeWakeSource;

    fn wait_for_wake(source: &RuntimeWakeSource) {
        let mut pollfd = libc::pollfd {
            fd: source.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: source retains the descriptor throughout this bounded wait.
        let ready = unsafe { libc::poll(&mut pollfd, 1, 1_000) };
        assert_eq!(ready, 1);
        assert_ne!(pollfd.revents & libc::POLLIN, 0);
        source.drain().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn tray_signal_publishes_state_then_wakes_overlay_owner() {
        let _guard = crate::unix_signals::test_signal_lock();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut signals = setup_signal_handlers(wake.handle()).unwrap();

        crate::unix_signals::deliver_signal_for_test(SIGUSR2);
        wait_for_wake(&wake);

        assert!(signals.take_tray_action_requested());
        assert!(signals.failure().is_none());
        signals.stop_and_join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn listener_failure_is_published_before_overlay_owner_wake() {
        let _guard = crate::unix_signals::test_signal_lock();
        let wake = RuntimeWakeSource::new().unwrap();
        let mut signals = setup_signal_handlers(wake.handle()).unwrap();

        signals.listener.inject_read_error(libc::EIO);
        wait_for_wake(&wake);

        let failure = signals.failure().expect("listener failure");
        assert!(failure.contains("os error Some(5)"), "{failure}");
        let stop_err = signals.stop_and_join().unwrap_err();
        assert!(stop_err.to_string().contains("failed before teardown"));
    }
}
