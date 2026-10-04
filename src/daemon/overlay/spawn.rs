use anyhow::anyhow;
use log::{info, warn};
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;

use crate::env_vars::PATH_ENV;

use super::launch::OverlayLaunchRequest;
use super::lifecycle::OverlayStartFailure;

use super::super::core::Daemon;
use super::super::types::OverlaySpawnCandidate;
#[cfg(feature = "tray")]
use super::super::types::OverlaySpawnErrorInfo;

impl Daemon {
    pub(super) fn record_overlay_spawn_failure(&mut self, message: String) {
        let backoff = self.overlay.record_spawn_failure();
        warn!(
            "Failed to spawn overlay process: {} (retry in {}s)",
            message,
            backoff.as_secs().max(1)
        );
        #[cfg(feature = "tray")]
        self.tray_status
            .set_overlay_error(Some(OverlaySpawnErrorInfo {
                message,
                next_retry_at: self.overlay.next_spawn_retry(),
            }));
    }

    pub(super) fn clear_overlay_spawn_error(&mut self) {
        self.overlay.clear_spawn_backoff();
        #[cfg(feature = "tray")]
        self.tray_status.set_overlay_error(None);
    }

    pub(super) fn overlay_spawn_candidates(&self) -> Vec<OverlaySpawnCandidate> {
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

    pub(super) fn spawn_overlay_process(
        &mut self,
        request: &OverlayLaunchRequest,
        candidates: &[OverlaySpawnCandidate],
    ) -> std::result::Result<(), OverlayStartFailure> {
        if candidates.is_empty() {
            return Err(OverlayStartFailure::BeforeAttempt(anyhow!(
                "No overlay spawn candidates available"
            )));
        }

        let mut failures = Vec::new();

        for candidate in candidates {
            match self.overlay.start(
                request,
                candidate,
                &self.session_resume_override,
                &self.instance_token,
            ) {
                Ok(pid) => {
                    info!(
                        "Overlay process started via {} (pid {pid}, startup_activation_token={})",
                        candidate.source,
                        request.activation_token().is_some()
                    );
                    return Ok(());
                }
                Err(OverlayStartFailure::BeforeAttempt(error)) => {
                    return Err(OverlayStartFailure::BeforeAttempt(error));
                }
                Err(OverlayStartFailure::Attempt(error)) => {
                    failures.push(format!(
                        "{} ({}) -> {error:#}",
                        candidate.source,
                        candidate.program.to_string_lossy()
                    ));
                }
            }
        }

        self.overlay.abort_start();
        warn!("Overlay spawn attempts failed: {}", failures.join("; "));
        Err(OverlayStartFailure::Attempt(anyhow!(
            "Unable to launch overlay process (tried current_exe/argv0/{PATH_ENV})"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::super::launch::{OverlayLaunch, build_overlay_launch};
    use super::*;

    fn build_test_launch(daemon: &mut Daemon) -> OverlayLaunch {
        let request = daemon.take_pending_launch();
        build_overlay_launch(&request, daemon.session_resume_override(), None)
    }

    fn launch_args(launch: &OverlayLaunch) -> Vec<String> {
        launch
            .arguments
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn build_overlay_command_includes_freeze_when_enabled() {
        let mut daemon = Daemon::new(Some("whiteboard".into()), false, None, None);
        daemon.set_freeze_on_show(true);

        let launch = build_test_launch(&mut daemon);

        assert_eq!(
            launch_args(&launch),
            vec!["--active", "--freeze", "--mode", "whiteboard"]
        );
    }

    #[test]
    fn build_overlay_command_uses_toggle_request_args() {
        for (exit_after_capture, no_exit_after_capture, expected_flag) in [
            (true, false, "--exit-after-capture"),
            (false, true, "--no-exit-after-capture"),
        ] {
            let mut daemon = Daemon::new(Some("whiteboard".into()), false, None, None);
            daemon.queue_overlay_launch(
                Some(crate::daemon::DaemonToggleRequest {
                    mode: Some("transparent".into()),
                    freeze: true,
                    exit_after_capture,
                    no_exit_after_capture,
                    ..Default::default()
                }),
                None,
            );

            let launch = build_test_launch(&mut daemon);

            assert_eq!(
                launch_args(&launch),
                vec![
                    "--active",
                    "--freeze",
                    expected_flag,
                    "--mode",
                    "transparent"
                ]
            );
        }
    }

    #[test]
    fn build_overlay_command_includes_initial_named_session_file() {
        let mut daemon = Daemon::new(
            Some("whiteboard".into()),
            false,
            None,
            Some(std::path::PathBuf::from("/tmp/lecture.wayscriber-session")),
        );

        let launch = build_test_launch(&mut daemon);

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
        daemon.queue_overlay_launch(
            Some(crate::daemon::DaemonToggleRequest {
                session_file: Some(std::path::PathBuf::from(
                    "/tmp/requested.wayscriber-session",
                )),
                ..Default::default()
            }),
            None,
        );

        let launch = build_test_launch(&mut daemon);

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
        let mut daemon = Daemon::new(Some("whiteboard".into()), false, None, None);

        let launch = build_test_launch(&mut daemon);

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
