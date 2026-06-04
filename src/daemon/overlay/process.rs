use anyhow::Result;
use log::{info, warn};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::super::core::Daemon;
use super::super::types::OverlayState;

impl Daemon {
    pub(super) fn terminate_overlay_process(&mut self) -> Result<()> {
        if let Some(mut child) = self.overlay_child.take() {
            let stop_started = Instant::now();
            let timeout = Duration::from_secs(2);
            info!(
                "Stopping overlay process (pid {}, graceful_timeout={:?})",
                child.id(),
                timeout
            );
            #[cfg(unix)]
            {
                if unsafe { libc::kill(child.id() as i32, libc::SIGTERM) } != 0 {
                    warn!(
                        "Failed to signal overlay process: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
            #[cfg(not(unix))]
            {
                if let Err(err) = child.kill() {
                    warn!("Failed to signal overlay process: {}", err);
                }
            }

            let deadline = Instant::now() + timeout;
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        info!(
                            "Overlay process exited with status {:?} after {:?}",
                            status,
                            stop_started.elapsed()
                        );
                        break;
                    }
                    Ok(None) => {
                        if Instant::now() >= deadline {
                            warn!(
                                "Overlay process did not exit after {:?}, sending SIGKILL",
                                stop_started.elapsed()
                            );
                            let _ = child.kill();
                            match child.wait() {
                                Ok(status) => warn!(
                                    "Overlay process killed with status {:?} after {:?}",
                                    status,
                                    stop_started.elapsed()
                                ),
                                Err(err) => warn!(
                                    "Failed to wait for killed overlay process after {:?}: {}",
                                    stop_started.elapsed(),
                                    err
                                ),
                            }
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => {
                        warn!("Failed to query overlay process status: {}", err);
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }
        }
        self.overlay_pid.store(0, Ordering::Release);
        self.active_named_session_file = None;
        Ok(())
    }

    pub(in crate::daemon) fn update_overlay_process_state(&mut self) -> Result<()> {
        if self.backend_runner.is_some() {
            return Ok(());
        }

        if let Some(child) = self.overlay_child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    info!("Overlay process exited with status {:?}", status);
                    self.overlay_child = None;
                    self.overlay_state = OverlayState::Hidden;
                    self.overlay_pid.store(0, Ordering::Release);
                    self.active_named_session_file = None;
                }
                Ok(None) => {}
                Err(err) => {
                    warn!("Failed to poll overlay process status: {}", err);
                }
            }
        }
        Ok(())
    }
}
