#[cfg(unix)]
use signal_hook::{
    consts::signal::{SIGINT, SIGTERM, SIGUSR1, SIGUSR2},
    iterator::Signals,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[cfg(unix)]
use std::thread;

pub(super) fn setup_signal_handlers() -> (Option<Arc<AtomicBool>>, Option<Arc<AtomicBool>>) {
    // Gracefully exit the overlay when external signals request termination
    #[cfg(unix)]
    {
        let exit_flag = Arc::new(AtomicBool::new(false));
        let tray_action_flag = Arc::new(AtomicBool::new(false));
        match Signals::new([SIGTERM, SIGINT, SIGUSR1, SIGUSR2]) {
            Ok(mut signals) => {
                let exit_flag_clone = Arc::clone(&exit_flag);
                let tray_action_flag_clone = Arc::clone(&tray_action_flag);
                thread::spawn(move || {
                    for sig in signals.forever() {
                        match sig {
                            SIGUSR2 => {
                                log::debug!("Overlay received SIGUSR2 for tray action");
                                tray_action_flag_clone.store(true, Ordering::Release);
                            }
                            _ => {
                                log::debug!(
                                    "Overlay received signal {}; scheduling graceful shutdown",
                                    sig
                                );
                                exit_flag_clone.store(true, Ordering::Release);
                            }
                        }
                    }
                });
                (Some(exit_flag), Some(tray_action_flag))
            }
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
