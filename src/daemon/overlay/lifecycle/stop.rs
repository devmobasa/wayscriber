use anyhow::{Context, Result};
use log::{info, warn};
use std::os::fd::AsFd;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::OverlayLifecycle;
use crate::daemon::protocol_v2::{BootClock, open_overlay_pidfd, wait_for_pidfd_exit};

impl OverlayLifecycle {
    fn terminate(&mut self) -> Result<()> {
        if let Some(pid) = self.child.display_pid() {
            let stop_started = Instant::now();
            let timeout = Duration::from_secs(2);
            info!(
                "Stopping overlay process (pid {}, graceful_timeout={:?})",
                pid, timeout
            );
            if let Err(err) = self.child.begin_stop() {
                warn!("Failed to signal overlay process: {err:#}");
            }

            // Wait on the child's pidfd rather than waking every 50ms to poll
            // it. A pidfd becomes readable exactly when its process exits, so
            // prompt exits are observed immediately and slow exits cost no
            // periodic wakeups. This termination path is still synchronous:
            // the daemon event loop remains occupied until the child exits or
            // the graceful timeout expires.
            let exit_watch = open_overlay_pidfd(pid).ok();
            let deadline = BootClock::now()?.checked_add(timeout)?;
            loop {
                match self.child.try_wait() {
                    Ok(Some(status)) => {
                        info!(
                            "Overlay process exited with status {:?} after {:?}",
                            status,
                            stop_started.elapsed()
                        );
                        break;
                    }
                    Ok(None) => {
                        if BootClock::now()? >= deadline {
                            warn!(
                                "Overlay process did not exit after {:?}, sending SIGKILL",
                                stop_started.elapsed()
                            );
                            let status = self
                                .child
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
                                let now = BootClock::now()?.as_nanos();
                                let remaining =
                                    Duration::from_nanos(deadline.as_nanos().saturating_sub(now));
                                let _ = wait_for_pidfd_exit(fd.as_fd(), remaining);
                            }
                            None => thread::sleep(Duration::from_millis(50)),
                        }
                    }
                    Err(err) => {
                        let forced = self.child.force_kill_and_wait();
                        return match forced {
                            Ok(_) => Err(err).context(
                                "broker ownership failed while querying overlay; child was forced down",
                            ),
                            Err(force_error) => Err(anyhow::anyhow!(
                                "broker ownership failed while querying overlay: {err:#}; \
                                 forced termination also failed: {force_error:#}"
                            )),
                        };
                    }
                }
            }
        }

        self.active.store(false, Ordering::Release);
        self.active_named_session_file = None;
        Ok(())
    }

    pub(in crate::daemon::overlay) fn hide(&mut self) -> Result<()> {
        self.terminate()?;
        self.mark_hidden();
        Ok(())
    }

    pub(in crate::daemon::overlay) fn poll_exit(&mut self) -> Result<()> {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                info!("Overlay process exited with status {:?}", status);
                self.mark_hidden();
            }
            Ok(None) => {}
            Err(err) => return Err(err).context("lost broker ownership of overlay child"),
        }
        Ok(())
    }
}
