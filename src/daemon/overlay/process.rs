use anyhow::{Context, Result};
use log::{info, warn};
use std::thread;
use std::time::{Duration, Instant};

use super::super::core::Daemon;
use super::super::types::OverlayState;

impl Daemon {
    pub(super) fn terminate_overlay_process(&mut self) -> Result<()> {
        if let Some(pid) = self.overlay_child.display_pid() {
            let stop_started = Instant::now();
            let timeout = Duration::from_secs(2);
            info!(
                "Stopping overlay process (pid {}, graceful_timeout={:?})",
                pid, timeout
            );
            if let Err(err) = self.overlay_child.begin_stop() {
                warn!("Failed to signal overlay process: {err:#}");
            }

            let deadline = super::super::protocol_v2::BootClock::now()?.checked_add(timeout)?;
            loop {
                match self.overlay_child.try_wait() {
                    Ok(Some(status)) => {
                        info!(
                            "Overlay process exited with status {:?} after {:?}",
                            status,
                            stop_started.elapsed()
                        );
                        break;
                    }
                    Ok(None) => {
                        if super::super::protocol_v2::BootClock::now()? >= deadline {
                            warn!(
                                "Overlay process did not exit after {:?}, sending SIGKILL",
                                stop_started.elapsed()
                            );
                            let status = self
                                .overlay_child
                                .force_kill_and_wait()
                                .context("lost broker ownership while forcing overlay shutdown")?;
                            warn!(
                                "Overlay process killed with status {:?} after {:?}",
                                status,
                                stop_started.elapsed()
                            );
                            break;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(err) => {
                        let forced = self.overlay_child.force_kill_and_wait();
                        return match forced {
                            Ok(_) => Err(err).context(
                                "broker ownership failed while querying overlay; child was forced down",
                            ),
                            Err(force_error) => Err(anyhow::anyhow!(
                                "broker ownership failed while querying overlay: {err:#}; forced termination also failed: {force_error:#}"
                            )),
                        };
                    }
                }
            }
        }
        self.overlay_active
            .store(false, std::sync::atomic::Ordering::Release);
        self.active_named_session_file = None;
        Ok(())
    }

    pub(in crate::daemon) fn update_overlay_process_state(&mut self) -> Result<()> {
        if self.backend_runner.is_some() {
            return Ok(());
        }

        match self.overlay_child.try_wait() {
            Ok(Some(status)) => {
                info!("Overlay process exited with status {:?}", status);
                self.overlay_state = OverlayState::Hidden;
                self.overlay_active
                    .store(false, std::sync::atomic::Ordering::Release);
                self.active_named_session_file = None;
            }
            Ok(None) => {}
            Err(err) => {
                return Err(err).context("lost broker ownership of overlay child");
            }
        }
        Ok(())
    }
}
