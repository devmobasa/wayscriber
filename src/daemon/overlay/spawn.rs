use anyhow::{Result, anyhow};
use log::{debug, info, warn};
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use crate::env_vars::{DESKTOP_STARTUP_ID_ENV, NO_DETACH_ENV, PATH_ENV, XDG_ACTIVATION_TOKEN_ENV};

use super::super::core::Daemon;
#[cfg(feature = "tray")]
use super::super::types::OverlaySpawnErrorInfo;
use super::super::types::{OverlaySpawnCandidate, OverlayState};

const OVERLAY_SPAWN_BACKOFF_BASE: Duration = Duration::from_secs(1);
const OVERLAY_SPAWN_BACKOFF_MAX: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq, Eq)]
struct OverlayLaunch {
    arguments: Vec<OsString>,
    environment: Vec<(OsString, Option<OsString>)>,
}

impl Daemon {
    fn overlay_spawn_backoff_duration(&self) -> Duration {
        let failures = self.overlay_spawn_failures.max(1);
        let shift = failures.saturating_sub(1).min(5);
        let base = OVERLAY_SPAWN_BACKOFF_BASE.as_secs().max(1);
        let secs = base.saturating_mul(1_u64 << shift);
        Duration::from_secs(secs.min(OVERLAY_SPAWN_BACKOFF_MAX.as_secs()))
    }

    pub(super) fn overlay_spawn_allowed(&mut self) -> bool {
        if let Some(next_retry) = self.overlay_spawn_next_retry {
            let now = Instant::now();
            if now < next_retry {
                if !self.overlay_spawn_backoff_logged {
                    let remaining = next_retry.saturating_duration_since(now);
                    warn!(
                        "Overlay spawn backoff active (retry in {}s)",
                        remaining.as_secs().max(1)
                    );
                    self.overlay_spawn_backoff_logged = true;
                }
                return false;
            }
        }
        self.overlay_spawn_backoff_logged = false;
        true
    }

    pub(super) fn record_overlay_spawn_failure(&mut self, message: String) {
        self.overlay_spawn_failures = self.overlay_spawn_failures.saturating_add(1);
        let backoff = self.overlay_spawn_backoff_duration();
        let next_retry_at = Instant::now() + backoff;
        self.overlay_spawn_next_retry = Some(next_retry_at);
        self.overlay_spawn_backoff_logged = false;
        warn!(
            "Failed to spawn overlay process: {} (retry in {}s)",
            message,
            backoff.as_secs().max(1)
        );
        #[cfg(feature = "tray")]
        self.tray_status
            .set_overlay_error(Some(OverlaySpawnErrorInfo {
                message,
                next_retry_at: Some(next_retry_at),
            }));
    }

    pub(super) fn clear_overlay_spawn_error(&mut self) {
        self.overlay_spawn_failures = 0;
        self.overlay_spawn_next_retry = None;
        self.overlay_spawn_backoff_logged = false;
        #[cfg(feature = "tray")]
        self.tray_status.set_overlay_error(None);
    }

    fn overlay_spawn_candidates(&self) -> Vec<OverlaySpawnCandidate> {
        let mut candidates = Vec::new();
        let mut seen = HashSet::<OsString>::new();

        if let Ok(exe) = env::current_exe() {
            if exe.is_file() {
                Self::push_spawn_candidate(&mut candidates, &mut seen, exe.into(), "current_exe");
            } else {
                warn!(
                    "Current executable path {} is not a file; falling back",
                    exe.display()
                );
            }
        } else {
            warn!("Failed to resolve current executable; falling back to argv0/{PATH_ENV}");
        }

        if let Some(arg0) = env::args_os().next() {
            let arg0_path = std::path::Path::new(&arg0);
            if arg0_path.to_string_lossy().contains('/') {
                if arg0_path.is_file() {
                    Self::push_spawn_candidate(&mut candidates, &mut seen, arg0, "argv0");
                } else {
                    warn!(
                        "argv0 path {} is not a file; falling back",
                        arg0_path.display()
                    );
                }
            } else {
                Self::push_spawn_candidate(&mut candidates, &mut seen, arg0, "argv0");
            }
        }

        Self::push_spawn_candidate(
            &mut candidates,
            &mut seen,
            OsString::from("wayscriber"),
            PATH_ENV,
        );

        candidates
    }

    fn push_spawn_candidate(
        candidates: &mut Vec<OverlaySpawnCandidate>,
        seen: &mut HashSet<OsString>,
        program: OsString,
        source: &'static str,
    ) {
        if seen.insert(program.clone()) {
            candidates.push(OverlaySpawnCandidate { program, source });
        }
    }

    fn build_overlay_launch(&self) -> OverlayLaunch {
        let mut arguments = vec![OsString::from("--active")];
        let request = self.pending_toggle_request.as_ref();
        if request.is_some_and(|request| request.freeze) || self.freeze_on_show {
            arguments.push("--freeze".into());
        }
        if request.is_some_and(|request| request.exit_after_capture) {
            arguments.push("--exit-after-capture".into());
        } else if request.is_some_and(|request| request.no_exit_after_capture) {
            arguments.push("--no-exit-after-capture".into());
        }
        // Overlay children launched by daemon are already backgrounded and tracked.
        // Prevent `--active` from spawning another detached grandchild process.
        let mut environment = vec![(OsString::from(NO_DETACH_ENV), Some("1".into()))];
        if let Some(generation) = self.overlay_child.generation() {
            environment.push((
                crate::env_vars::OVERLAY_CHILD_GENERATION_ENV.into(),
                Some(generation.into()),
            ));
        }
        if let Some(token) = self.pending_activation_token.as_deref() {
            environment.push((XDG_ACTIVATION_TOKEN_ENV.into(), Some(token.into())));
            environment.push((DESKTOP_STARTUP_ID_ENV.into(), Some(token.into())));
        } else {
            environment.push((XDG_ACTIVATION_TOKEN_ENV.into(), None));
            environment.push((DESKTOP_STARTUP_ID_ENV.into(), None));
        }
        if let Some(request_override) =
            request.and_then(|request| request.session_resume_override())
        {
            match request_override {
                true => environment.push((crate::RESUME_SESSION_ENV.into(), Some("on".into()))),
                false => environment.push((crate::RESUME_SESSION_ENV.into(), Some("off".into()))),
            }
        } else {
            environment.push((
                crate::RESUME_SESSION_ENV.into(),
                self.session_resume_override()
                    .map(|enabled| if enabled { "on".into() } else { "off".into() }),
            ));
        }
        if let Some(mode) = request
            .and_then(|request| request.mode.as_ref())
            .or(self.initial_mode.as_ref())
        {
            arguments.push("--mode".into());
            arguments.push(mode.into());
        }
        if let Some(path) = self.effective_named_session_file() {
            arguments.push("--session-file".into());
            arguments.push(path.into_os_string());
        }
        OverlayLaunch {
            arguments,
            environment,
        }
    }

    pub(super) fn spawn_overlay_process(&mut self) -> Result<()> {
        let candidates = self.overlay_spawn_candidates();
        if candidates.is_empty() {
            return Err(anyhow!("No overlay spawn candidates available"));
        }

        let mut failures = Vec::new();

        for candidate in candidates {
            self.overlay_child.reserve()?;
            let had_activation_token = self.pending_activation_token.is_some();
            debug!(
                "Attempting overlay spawn via {} ({})",
                candidate.source,
                candidate.program.to_string_lossy()
            );
            let launch = self.build_overlay_launch();
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
                let pid = child.id();
                self.overlay_child.start(child)?;
                self.overlay_child
                    .wait_until_ready(Duration::from_secs(5), &self.instance_token)?;
                Ok(pid)
            })();
            match attempt {
                Ok(pid) => {
                    self.overlay_active
                        .store(true, std::sync::atomic::Ordering::Release);
                    self.overlay_state = OverlayState::Visible;
                    self.active_named_session_file = self.effective_named_session_file();
                    self.pending_activation_token = None;
                    info!(
                        "Overlay process started via {} (pid {pid}, startup_activation_token={})",
                        candidate.source, had_activation_token
                    );
                    return Ok(());
                }
                Err(error) => {
                    self.overlay_child.abort_reservation();
                    failures.push(format!(
                        "{} ({}) -> {error:#}",
                        candidate.source,
                        candidate.program.to_string_lossy(),
                    ));
                }
            }
        }

        self.pending_activation_token = None;
        self.overlay_child.abort_reservation();
        warn!("Overlay spawn attempts failed: {}", failures.join("; "));
        Err(anyhow!(
            "Unable to launch overlay process (tried current_exe/argv0/{PATH_ENV})"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch_args(launch: &OverlayLaunch) -> Vec<String> {
        launch
            .arguments
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn backoff_duration_grows_and_caps() {
        let mut daemon = Daemon::new(None, false, None, None);

        daemon.overlay_spawn_failures = 1;
        assert_eq!(
            daemon.overlay_spawn_backoff_duration(),
            Duration::from_secs(1)
        );

        daemon.overlay_spawn_failures = 2;
        assert_eq!(
            daemon.overlay_spawn_backoff_duration(),
            Duration::from_secs(2)
        );

        daemon.overlay_spawn_failures = 5;
        assert_eq!(
            daemon.overlay_spawn_backoff_duration(),
            Duration::from_secs(16)
        );

        daemon.overlay_spawn_failures = 6;
        assert_eq!(
            daemon.overlay_spawn_backoff_duration(),
            Duration::from_secs(30)
        );
    }

    #[test]
    fn overlay_spawn_allowed_honors_retry_window() {
        let mut daemon = Daemon::new(None, false, None, None);
        daemon.overlay_spawn_next_retry = Some(Instant::now() + Duration::from_secs(2));
        daemon.overlay_spawn_backoff_logged = false;

        assert!(!daemon.overlay_spawn_allowed());
        assert!(daemon.overlay_spawn_backoff_logged);

        daemon.overlay_spawn_next_retry = Some(Instant::now() - Duration::from_secs(1));
        assert!(daemon.overlay_spawn_allowed());
        assert!(!daemon.overlay_spawn_backoff_logged);
    }

    #[test]
    fn build_overlay_command_includes_freeze_when_enabled() {
        let mut daemon = Daemon::new(Some("whiteboard".into()), false, None, None);
        daemon.set_freeze_on_show(true);

        let launch = daemon.build_overlay_launch();

        assert_eq!(
            launch_args(&launch),
            vec!["--active", "--freeze", "--mode", "whiteboard"]
        );
    }

    #[test]
    fn build_overlay_command_uses_toggle_request_args() {
        let mut daemon = Daemon::new(Some("whiteboard".into()), false, None, None);
        daemon.pending_toggle_request = Some(crate::daemon::DaemonToggleRequest {
            mode: Some("transparent".into()),
            freeze: true,
            exit_after_capture: true,
            ..Default::default()
        });

        let launch = daemon.build_overlay_launch();

        assert_eq!(
            launch_args(&launch),
            vec![
                "--active",
                "--freeze",
                "--exit-after-capture",
                "--mode",
                "transparent"
            ]
        );
    }

    #[test]
    fn build_overlay_command_includes_initial_named_session_file() {
        let daemon = Daemon::new(
            Some("whiteboard".into()),
            false,
            None,
            Some(std::path::PathBuf::from("/tmp/lecture.wayscriber-session")),
        );

        let launch = daemon.build_overlay_launch();

        assert_eq!(
            launch_args(&launch),
            vec![
                "--active",
                "--mode",
                "whiteboard",
                "--session-file",
                "/tmp/lecture.wayscriber-session"
            ]
        );
    }

    #[test]
    fn build_overlay_command_request_session_file_overrides_initial_named_session_file() {
        let mut daemon = Daemon::new(
            Some("whiteboard".into()),
            false,
            None,
            Some(std::path::PathBuf::from("/tmp/default.wayscriber-session")),
        );
        daemon.pending_toggle_request = Some(crate::daemon::DaemonToggleRequest {
            session_file: Some(std::path::PathBuf::from(
                "/tmp/requested.wayscriber-session",
            )),
            ..Default::default()
        });

        let launch = daemon.build_overlay_launch();

        assert_eq!(
            launch_args(&launch),
            vec![
                "--active",
                "--mode",
                "whiteboard",
                "--session-file",
                "/tmp/requested.wayscriber-session"
            ]
        );
    }

    #[test]
    fn build_overlay_command_omits_freeze_by_default() {
        let daemon = Daemon::new(Some("whiteboard".into()), false, None, None);

        let launch = daemon.build_overlay_launch();

        assert_eq!(
            launch_args(&launch),
            vec!["--active", "--mode", "whiteboard"]
        );
    }

    #[test]
    fn push_spawn_candidate_deduplicates_programs() {
        let mut candidates = Vec::new();
        let mut seen = HashSet::<OsString>::new();

        Daemon::push_spawn_candidate(
            &mut candidates,
            &mut seen,
            OsString::from("wayscriber"),
            super::PATH_ENV,
        );
        Daemon::push_spawn_candidate(
            &mut candidates,
            &mut seen,
            OsString::from("wayscriber"),
            "argv0",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, super::PATH_ENV);
    }
}
