use anyhow::{Context, Result};
use log::{info, warn};
use std::os::fd::AsFd;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::OverlayLifecycle;
use crate::daemon::protocol_v2::{
    BootClock, ReportedSession, open_overlay_pidfd, wait_for_pidfd_exit,
};

/// A stop that failed. A child forced down after its broker failed was still
/// retired, so the session it last reported comes with the error.
pub(in crate::daemon) struct StopFailure {
    pub(in crate::daemon) session: Option<ReportedSession>,
    pub(in crate::daemon) error: anyhow::Error,
}

impl From<anyhow::Error> for StopFailure {
    fn from(error: anyhow::Error) -> Self {
        Self {
            session: None,
            error,
        }
    }
}

impl From<std::io::Error> for StopFailure {
    fn from(error: std::io::Error) -> Self {
        anyhow::Error::from(error).into()
    }
}

impl OverlayLifecycle {
    /// Stops the child, returning the session it last reported.
    fn terminate(&mut self) -> std::result::Result<Option<ReportedSession>, StopFailure> {
        let mut session = None;
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
                    Ok(Some(exit)) => {
                        info!(
                            "Overlay process exited with status {:?} after {:?}",
                            exit.status,
                            stop_started.elapsed()
                        );
                        session = exit.session;
                        break;
                    }
                    Ok(None) => {
                        if BootClock::now()? >= deadline {
                            warn!(
                                "Overlay process did not exit after {:?}, sending SIGKILL",
                                stop_started.elapsed()
                            );
                            let exit = self
                                .child
                                .force_kill_and_wait()
                                .context("lost broker ownership while forcing overlay shutdown")?;
                            warn!(
                                "Overlay process killed with status {:?} after {:?}",
                                exit.status,
                                stop_started.elapsed()
                            );
                            session = exit.session;
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
                        return Err(match self.child.force_kill_and_wait() {
                            Ok(exit) => StopFailure {
                                session: exit.session,
                                error: err.context(
                                    "broker ownership failed while querying overlay; child was forced down",
                                ),
                            },
                            Err(force_error) => anyhow::anyhow!(
                                "broker ownership failed while querying overlay: {err:#}; \
                                 forced termination also failed: {force_error:#}"
                            )
                            .into(),
                        });
                    }
                }
            }
        }

        self.active.store(false, Ordering::Release);
        self.active_named_session_file = None;
        Ok(session)
    }

    /// Stops the overlay, returning the session its child last reported.
    pub(in crate::daemon::overlay) fn hide(
        &mut self,
    ) -> std::result::Result<Option<ReportedSession>, StopFailure> {
        let session = self.terminate()?;
        self.mark_hidden();
        Ok(session)
    }

    /// Retires a child that exited on its own, returning the session it last
    /// reported.
    pub(in crate::daemon::overlay) fn poll_exit(&mut self) -> Result<Option<ReportedSession>> {
        match self.child.try_wait() {
            Ok(Some(exit)) => {
                info!("Overlay process exited with status {:?}", exit.status);
                self.mark_hidden();
                Ok(exit.session)
            }
            Ok(None) => Ok(None),
            Err(err) => Err(err).context("lost broker ownership of overlay child"),
        }
    }
}
