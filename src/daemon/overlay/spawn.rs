use anyhow::{Result, anyhow};
use log::{debug, info, warn};
use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::super::core::Daemon;
#[cfg(feature = "tray")]
use super::super::types::OverlaySpawnErrorInfo;
use super::super::types::{OverlaySpawnCandidate, OverlayState};

const OVERLAY_SPAWN_BACKOFF_BASE: Duration = Duration::from_secs(1);
const OVERLAY_SPAWN_BACKOFF_MAX: Duration = Duration::from_secs(30);

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
            warn!("Failed to resolve current executable; falling back to argv0/PATH");
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
            "PATH",
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

    fn build_overlay_command(&self, program: &OsStr) -> Command {
        let mut command = Command::new(program);
        command.arg("--active");
        let request = self.pending_toggle_request.as_ref();
        if request.is_some_and(|request| request.freeze) || self.freeze_on_show {
            command.arg("--freeze");
        }
        if request.is_some_and(|request| request.exit_after_capture) {
            command.arg("--exit-after-capture");
        } else if request.is_some_and(|request| request.no_exit_after_capture) {
            command.arg("--no-exit-after-capture");
        }
        // Overlay children launched by daemon are already backgrounded and tracked.
        // Prevent `--active` from spawning another detached grandchild process.
        command.env("WAYSCRIBER_NO_DETACH", "1");
        if let Some(token) = self.pending_activation_token.as_deref() {
            command.env("XDG_ACTIVATION_TOKEN", token);
            command.env("DESKTOP_STARTUP_ID", token);
        } else {
            command.env_remove("XDG_ACTIVATION_TOKEN");
            command.env_remove("DESKTOP_STARTUP_ID");
        }
        if let Some(request_override) =
            request.and_then(|request| request.session_resume_override())
        {
            match request_override {
                true => command.env(crate::RESUME_SESSION_ENV, "on"),
                false => command.env(crate::RESUME_SESSION_ENV, "off"),
            };
        } else {
            self.apply_session_override_env(&mut command);
        }
        if let Some(mode) = request
            .and_then(|request| request.mode.as_ref())
            .or(self.initial_mode.as_ref())
        {
            command.arg("--mode").arg(mode);
        }
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command
    }

    pub(super) fn spawn_overlay_process(&mut self) -> Result<()> {
        let candidates = self.overlay_spawn_candidates();
        if candidates.is_empty() {
            return Err(anyhow!("No overlay spawn candidates available"));
        }

        let mut failures = Vec::new();

        for candidate in candidates {
            let had_activation_token = self.pending_activation_token.is_some();
            debug!(
                "Attempting overlay spawn via {} ({})",
                candidate.source,
                candidate.program.to_string_lossy()
            );
            let mut command = self.build_overlay_command(&candidate.program);
            match command.spawn() {
                Ok(child) => {
                    let pid = child.id();
                    self.overlay_pid.store(pid, Ordering::Release);
                    self.overlay_child = Some(child);
                    self.overlay_state = OverlayState::Visible;
                    self.pending_activation_token = None;
                    info!(
                        "Overlay process started via {} (pid {pid}, startup_activation_token={})",
                        candidate.source, had_activation_token
                    );
                    return Ok(());
                }
                Err(err) => {
                    failures.push(format!(
                        "{} ({}) -> {}",
                        candidate.source,
                        candidate.program.to_string_lossy(),
                        err
                    ));
                }
            }
        }

        self.pending_activation_token = None;
        warn!("Overlay spawn attempts failed: {}", failures.join("; "));
        Err(anyhow!(
            "Unable to launch overlay process (tried current_exe/argv0/PATH)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn backoff_duration_grows_and_caps() {
        let mut daemon = Daemon::new(None, false, None);

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
        let mut daemon = Daemon::new(None, false, None);
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
        let mut daemon = Daemon::new(Some("whiteboard".into()), false, None);
        daemon.set_freeze_on_show(true);

        let command = daemon.build_overlay_command(OsStr::new("wayscriber"));

        assert_eq!(
            command_args(&command),
            vec!["--active", "--freeze", "--mode", "whiteboard"]
        );
    }

    #[test]
    fn build_overlay_command_uses_toggle_request_args() {
        let mut daemon = Daemon::new(Some("whiteboard".into()), false, None);
        daemon.pending_toggle_request = Some(crate::daemon::DaemonToggleRequest {
            mode: Some("transparent".into()),
            freeze: true,
            exit_after_capture: true,
            ..Default::default()
        });

        let command = daemon.build_overlay_command(OsStr::new("wayscriber"));

        assert_eq!(
            command_args(&command),
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
    fn build_overlay_command_omits_freeze_by_default() {
        let daemon = Daemon::new(Some("whiteboard".into()), false, None);

        let command = daemon.build_overlay_command(OsStr::new("wayscriber"));

        assert_eq!(
            command_args(&command),
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
            "PATH",
        );
        Daemon::push_spawn_candidate(
            &mut candidates,
            &mut seen,
            OsString::from("wayscriber"),
            "argv0",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, "PATH");
    }
}
