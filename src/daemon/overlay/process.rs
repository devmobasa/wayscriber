use anyhow::{Context, Result};
use log::{info, warn};
use std::os::fd::AsFd;
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

            // Wait on the child's pidfd rather than waking every 50ms to poll
            // it. A pidfd becomes readable exactly when its process exits, so
            // prompt exits are observed immediately and slow exits cost no
            // periodic wakeups. This termination path is still synchronous:
            // the daemon event loop remains occupied until the child exits or
            // the graceful timeout expires.
            let exit_watch = super::super::protocol_v2::open_overlay_pidfd(pid).ok();
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
                        // Without a pidfd (the child raced us to exit, or the
                        // open failed) fall back to the original pacing.
                        match exit_watch.as_ref() {
                            Some(fd) => {
                                let now = super::super::protocol_v2::BootClock::now()?.as_nanos();
                                let remaining =
                                    Duration::from_nanos(deadline.as_nanos().saturating_sub(now));
                                let _ = super::super::protocol_v2::wait_for_pidfd_exit(
                                    fd.as_fd(),
                                    remaining,
                                );
                            }
                            None => thread::sleep(Duration::from_millis(50)),
                        }
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
