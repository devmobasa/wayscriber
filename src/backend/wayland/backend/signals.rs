#[cfg(unix)]
use libc::{SIGINT, SIGTERM, SIGUSR1, SIGUSR2};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) fn setup_signal_handlers() -> (Option<Arc<AtomicBool>>, Option<Arc<AtomicBool>>) {
    // Gracefully exit the overlay when external signals request termination
    #[cfg(unix)]
    {
        let exit_flag = Arc::new(AtomicBool::new(false));
        let tray_action_flag = Arc::new(AtomicBool::new(false));
        let signal_exit_flag = Arc::clone(&exit_flag);
        let signal_tray_action_flag = Arc::clone(&tray_action_flag);
        match crate::unix_signals::spawn_listener(
            &[SIGTERM, SIGINT, SIGUSR1, SIGUSR2],
            move |sig| {
                match sig {
                    SIGUSR1 => {
                        // SIGUSR1 is reserved for daemon toggle; ignore in overlay.
                        log::debug!("Overlay received SIGUSR1; ignoring");
                    }
                    SIGUSR2 => {
                        log::debug!("Overlay received SIGUSR2 for tray action");
                        signal_tray_action_flag.store(true, Ordering::Release);
                    }
                    _ => {
                        log::debug!(
                            "Overlay received signal {}; scheduling graceful shutdown",
                            sig
                        );
                        signal_exit_flag.store(true, Ordering::Release);
                    }
                }
            },
        ) {
            Ok(_) => (Some(exit_flag), Some(tray_action_flag)),
            Err(err) => {
                log::warn!("Failed to register overlay signal handlers: {}", err);
                (Some(exit_flag), Some(tray_action_flag))
            }
        }
    }

    #[cfg(not(unix))]
    {
        (None, None)
    }
}
