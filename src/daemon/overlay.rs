use anyhow::{Result, anyhow};
use log::{debug, info, warn};
use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use crate::{runtime_session_override, set_runtime_session_override};

use super::core::Daemon;
#[cfg(feature = "tray")]
use super::types::OverlaySpawnErrorInfo;
use super::types::{OverlaySpawnCandidate, OverlayState};

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

    fn overlay_spawn_allowed(&mut self) -> bool {
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

    fn record_overlay_spawn_failure(&mut self, message: String) {
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

    fn clear_overlay_spawn_error(&mut self) {
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

    /// Toggle overlay visibility
    pub(super) fn toggle_overlay(&mut self) -> Result<()> {
        match self.overlay_state {
            OverlayState::Hidden => {
                info!("Showing overlay");
                self.show_overlay()?;
            }
            OverlayState::Visible => {
                info!("Hiding overlay");
                self.hide_overlay()?;
            }
        }
        Ok(())
    }

    /// Show overlay (create layer surface and enter drawing mode)
    pub(super) fn show_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Visible {
            debug!("Overlay already visible");
            return Ok(());
        }

        if let Some(runner) = self.backend_runner.clone() {
            self.overlay_state = OverlayState::Visible;
            info!("Overlay state set to Visible");
            self.clear_overlay_spawn_error();
            let previous_override = runtime_session_override();
            set_runtime_session_override(self.session_resume_override());
            let result = runner(self.initial_mode.clone());
            set_runtime_session_override(previous_override);
            self.overlay_state = OverlayState::Hidden;
            info!("Overlay closed, back to daemon mode");
            return result;
        }

        if !self.overlay_spawn_allowed() {
            return Ok(());
        }

        if let Err(err) = self.spawn_overlay_process() {
            self.record_overlay_spawn_failure(err.to_string());
            return Err(err);
        }
        self.clear_overlay_spawn_error();
        Ok(())
    }

    /// Hide overlay (destroy layer surface, return to hidden state)
    pub(super) fn hide_overlay(&mut self) -> Result<()> {
        if self.overlay_state == OverlayState::Hidden {
            debug!("Overlay already hidden");
            return Ok(());
        }

        if self.backend_runner.is_some() {
            // Internal runner does not keep additional state to tear down
            debug!("Internal backend runner hidden");
            self.overlay_state = OverlayState::Hidden;
            return Ok(());
        }

        self.terminate_overlay_process()?;
        self.overlay_state = OverlayState::Hidden;
        Ok(())
    }

    fn spawn_overlay_process(&mut self) -> Result<()> {
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

    fn terminate_overlay_process(&mut self) -> Result<()> {
        if let Some(mut child) = self.overlay_child.take() {
            info!("Stopping overlay process (pid {})", child.id());
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

            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        info!("Overlay process exited with status {:?}", status);
                        break;
                    }
                    Ok(None) => {
                        if Instant::now() >= deadline {
                            warn!("Overlay process did not exit, sending SIGKILL");
                            let _ = child.kill();
                            let _ = child.wait();
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
        Ok(())
    }

    pub(super) fn update_overlay_process_state(&mut self) -> Result<()> {
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
