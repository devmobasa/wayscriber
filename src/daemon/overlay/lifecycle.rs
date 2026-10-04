use std::os::fd::{AsRawFd, BorrowedFd};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use log::{debug, info, warn};

use super::super::protocol_v2::OverlayChildOwner;
use super::super::types::{BackendRunner, OverlaySpawnCandidate, OverlayState};
use super::launch::{OverlayLaunchRequest, build_overlay_launch};

mod stop;

const SPAWN_BACKOFF_BASE: Duration = Duration::from_secs(1);
const SPAWN_BACKOFF_MAX: Duration = Duration::from_secs(30);

pub(super) enum OverlayStartFailure {
    BeforeAttempt(anyhow::Error),
    Attempt(anyhow::Error),
}

pub(in crate::daemon) struct OverlayLifecycle {
    state: OverlayState,
    child: OverlayChildOwner,
    active: Arc<AtomicBool>,
    active_named_session_file: Option<PathBuf>,
    spawn_failures: u32,
    next_spawn_retry: Option<Instant>,
    backoff_logged: bool,
}

impl Default for OverlayLifecycle {
    fn default() -> Self {
        Self {
            state: OverlayState::Hidden,
            child: OverlayChildOwner::default(),
            active: Arc::new(AtomicBool::new(false)),
            active_named_session_file: None,
            spawn_failures: 0,
            next_spawn_retry: None,
            backoff_logged: false,
        }
    }
}

impl OverlayLifecycle {
    pub(in crate::daemon) fn state(&self) -> OverlayState {
        self.state
    }

    pub(in crate::daemon) fn active_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.active)
    }

    pub(in crate::daemon) fn poll_fd(&self) -> Option<BorrowedFd<'_>> {
        self.child.poll_fd()
    }

    pub(in crate::daemon) fn signal(&self, signal: i32) -> Result<()> {
        self.child.signal(signal)
    }

    pub(in crate::daemon) fn active_named_session_file(&self) -> Option<&Path> {
        self.active_named_session_file.as_deref()
    }

    pub(super) fn start(
        &mut self,
        request: &OverlayLaunchRequest,
        candidate: &OverlaySpawnCandidate,
        resume_override: &AtomicU8,
        daemon_token: &str,
    ) -> std::result::Result<u32, OverlayStartFailure> {
        self.child
            .reserve()
            .map_err(OverlayStartFailure::BeforeAttempt)?;
        debug!(
            "Attempting overlay spawn via {} ({})",
            candidate.source,
            candidate.program.to_string_lossy()
        );
        let launch = build_overlay_launch(
            request,
            crate::decode_session_override(resume_override.load(Ordering::Acquire)),
            self.child.generation(),
        );

        let attempt = (|| -> Result<u32> {
            let daemon_watchdog = super::super::protocol_v2::open_daemon_watchdog()?;
            let child = crate::process_broker::current()?.spawn_with_watchdog(
                crate::process_broker::HelperKind::Overlay,
                crate::process_broker::HelperLifetime::OwnedChild,
                &candidate.program,
                &launch.arguments,
                launch.environment,
                daemon_watchdog.as_raw_fd(),
            )?;
            self.child.start(child)?;
            self.child
                .wait_until_ready(Duration::from_secs(5), daemon_token)?;
            self.mark_shown(request.named_session_file().map(Path::to_path_buf))
        })();
        match attempt {
            Ok(pid) => Ok(pid),
            Err(error) => {
                self.child.abort_reservation();
                Err(OverlayStartFailure::Attempt(error))
            }
        }
    }

    fn mark_shown(&mut self, named_session_file: Option<PathBuf>) -> Result<u32> {
        let pid = self
            .child
            .display_pid()
            .context("cannot show an overlay without an owned child")?;
        self.active.store(true, Ordering::Release);
        self.state = OverlayState::Visible;
        self.active_named_session_file = named_session_file;
        Ok(pid)
    }

    pub(super) fn mark_hidden(&mut self) {
        debug_assert!(self.child.display_pid().is_none());
        self.state = OverlayState::Hidden;
        self.active_named_session_file = None;
        self.active.store(false, Ordering::Release);
    }

    // The existing in-process backend does not own a broker child or advertise
    // an active child to the tray/update watcher. Keep that distinction explicit.
    pub(super) fn run_backend(
        &mut self,
        runner: &BackendRunner,
        request: &OverlayLaunchRequest,
        resume_default: Option<bool>,
    ) -> Result<()> {
        self.state = OverlayState::Visible;
        self.active_named_session_file = request.named_session_file().map(Path::to_path_buf);
        info!("Overlay state set to Visible");
        let previous_override = crate::runtime_session_override();
        crate::set_runtime_session_override(request.session_resume_override(resume_default));

        let result = runner(request.mode().map(str::to_owned));

        crate::set_runtime_session_override(previous_override);
        self.state = OverlayState::Hidden;
        self.active_named_session_file = None;
        info!("Overlay closed, back to daemon mode");

        result
    }

    fn spawn_backoff_duration(&self) -> Duration {
        let failures = self.spawn_failures.max(1);
        let shift = failures.saturating_sub(1).min(5);
        let base = SPAWN_BACKOFF_BASE.as_secs().max(1);
        let secs = base.saturating_mul(1_u64 << shift);
        Duration::from_secs(secs.min(SPAWN_BACKOFF_MAX.as_secs()))
    }

    pub(super) fn spawn_backoff_remaining(&mut self) -> Option<Duration> {
        if let Some(next_retry) = self.next_spawn_retry {
            let now = Instant::now();
            if now < next_retry {
                let remaining = next_retry.saturating_duration_since(now);
                if !self.backoff_logged {
                    warn!(
                        "Overlay spawn backoff active (retry in {}s)",
                        remaining.as_secs().max(1)
                    );
                    self.backoff_logged = true;
                }
                return Some(remaining);
            }
        }

        self.backoff_logged = false;
        None
    }

    pub(in crate::daemon) fn record_spawn_failure(&mut self) -> Duration {
        self.spawn_failures = self.spawn_failures.saturating_add(1);
        let backoff = self.spawn_backoff_duration();
        self.next_spawn_retry = Some(Instant::now() + backoff);
        self.backoff_logged = false;

        backoff
    }

    #[cfg(feature = "tray")]
    pub(super) fn next_spawn_retry(&self) -> Option<Instant> {
        self.next_spawn_retry
    }

    pub(super) fn clear_spawn_backoff(&mut self) {
        self.spawn_failures = 0;
        self.next_spawn_retry = None;
        self.backoff_logged = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_duration_grows_and_caps() {
        let mut lifecycle = OverlayLifecycle::default();
        for expected in [1, 2, 4, 8, 16, 30, 30] {
            assert_eq!(
                lifecycle.record_spawn_failure(),
                Duration::from_secs(expected)
            );
        }

        lifecycle.clear_spawn_backoff();
        assert!(lifecycle.spawn_backoff_remaining().is_none());
        assert_eq!(lifecycle.record_spawn_failure(), Duration::from_secs(1));
    }

    #[test]
    fn overlay_spawn_backoff_honors_retry_window() {
        let mut lifecycle = OverlayLifecycle {
            next_spawn_retry: Some(Instant::now() + Duration::from_secs(2)),
            ..Default::default()
        };
        assert!(lifecycle.spawn_backoff_remaining().is_some());
        assert!(lifecycle.backoff_logged);

        lifecycle.next_spawn_retry = Some(Instant::now() - Duration::from_secs(1));
        assert!(lifecycle.spawn_backoff_remaining().is_none());
        assert!(!lifecycle.backoff_logged);
    }

    #[test]
    fn showing_requires_an_owned_child() {
        let mut lifecycle = OverlayLifecycle::default();
        assert!(lifecycle.mark_shown(None).is_err());
        assert_eq!(lifecycle.state(), OverlayState::Hidden);
        assert!(!lifecycle.active_flag().load(Ordering::Acquire));
    }
}
