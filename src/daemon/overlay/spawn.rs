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
        self.apply_session_override_env(&mut command);
        if let Some(mode) = &self.initial_mode {
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
                    info!(
                        "Overlay process started via {} (pid {pid})",
                        candidate.source
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

        warn!("Overlay spawn attempts failed: {}", failures.join("; "));
        Err(anyhow!(
            "Unable to launch overlay process (tried current_exe/argv0/PATH)"
        ))
    }
}
