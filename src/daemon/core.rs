use anyhow::{Context, Result};
use log::{info, warn};
use signal_hook::consts::signal::{SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(test)]
use crate::SESSION_OVERRIDE_FOLLOW_CONFIG;
use crate::paths::daemon_lock_file;
use crate::session::try_lock_exclusive;
use crate::{RESUME_SESSION_ENV, decode_session_override, encode_session_override};

use super::tray::start_system_tray;
#[cfg(feature = "tray")]
use super::types::TrayStatusShared;
use super::types::{AlreadyRunningError, BackendRunner, OverlayState};

pub struct Daemon {
    pub(super) overlay_state: OverlayState,
    pub(super) should_quit: Arc<AtomicBool>,
    pub(super) toggle_requested: Arc<AtomicBool>,
    pub(super) initial_mode: Option<String>,
    pub(super) tray_enabled: bool,
    pub(super) backend_runner: Option<Arc<BackendRunner>>,
    pub(super) tray_thread: Option<JoinHandle<()>>,
    pub(super) overlay_child: Option<Child>,
    pub(super) overlay_pid: Arc<AtomicU32>,
    pub(super) session_resume_override: Arc<AtomicU8>,
    pub(super) lock_file: Option<std::fs::File>,
    pub(super) overlay_spawn_failures: u32,
    pub(super) overlay_spawn_next_retry: Option<std::time::Instant>,
    pub(super) overlay_spawn_backoff_logged: bool,
    #[cfg(feature = "tray")]
    pub(super) tray_status: Arc<TrayStatusShared>,
}

impl Daemon {
    pub fn new(
        initial_mode: Option<String>,
        tray_enabled: bool,
        session_resume_override: Option<bool>,
    ) -> Self {
        let override_state = Arc::new(AtomicU8::new(encode_session_override(
            session_resume_override,
        )));
        Self {
            overlay_state: OverlayState::Hidden,
            should_quit: Arc::new(AtomicBool::new(false)),
            toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
            tray_enabled,
            backend_runner: None,
            tray_thread: None,
            overlay_child: None,
            overlay_pid: Arc::new(AtomicU32::new(0)),
            session_resume_override: override_state,
            lock_file: None,
            overlay_spawn_failures: 0,
            overlay_spawn_next_retry: None,
            overlay_spawn_backoff_logged: false,
            #[cfg(feature = "tray")]
            tray_status: Arc::new(TrayStatusShared::new()),
        }
    }

    #[cfg(test)]
    fn with_backend_runner_internal(
        initial_mode: Option<String>,
        backend_runner: Arc<BackendRunner>,
    ) -> Self {
        let override_state = Arc::new(AtomicU8::new(SESSION_OVERRIDE_FOLLOW_CONFIG));
        Self {
            overlay_state: OverlayState::Hidden,
            should_quit: Arc::new(AtomicBool::new(false)),
            toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
            tray_enabled: true,
            backend_runner: Some(backend_runner),
            tray_thread: None,
            overlay_child: None,
            overlay_pid: Arc::new(AtomicU32::new(0)),
            session_resume_override: override_state,
            lock_file: None,
            overlay_spawn_failures: 0,
            overlay_spawn_next_retry: None,
            overlay_spawn_backoff_logged: false,
            #[cfg(feature = "tray")]
            tray_status: Arc::new(TrayStatusShared::new()),
        }
    }

    #[cfg(test)]
    pub fn with_backend_runner(
        initial_mode: Option<String>,
        backend_runner: Arc<BackendRunner>,
    ) -> Self {
        Self::with_backend_runner_internal(initial_mode, backend_runner)
    }

    pub(super) fn session_resume_override(&self) -> Option<bool> {
        decode_session_override(self.session_resume_override.load(Ordering::Acquire))
    }

    pub(super) fn apply_session_override_env(&self, command: &mut Command) {
        match self.session_resume_override() {
            Some(true) => {
                command.env(RESUME_SESSION_ENV, "on");
            }
            Some(false) => {
                command.env(RESUME_SESSION_ENV, "off");
            }
            None => {
                command.env_remove(RESUME_SESSION_ENV);
            }
        }
    }

    fn acquire_daemon_lock(&mut self) -> Result<()> {
        let lock_path = daemon_lock_file();
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create runtime directory {}", parent.display())
            })?;
        }

        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("failed to open daemon lock {}", lock_path.display()))?;

        match try_lock_exclusive(&lock_file) {
            Ok(()) => {
                self.lock_file = Some(lock_file);
                Ok(())
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => Err(AlreadyRunningError.into()),
            Err(err) => Err(err).context("failed to lock daemon instance"),
        }
    }

    /// Run daemon with signal handling
    pub fn run(&mut self) -> Result<()> {
        info!("Starting wayscriber daemon");
        info!("Send SIGUSR1 to toggle overlay (e.g., pkill -SIGUSR1 wayscriber)");
        info!("Configure Hyprland: bind = SUPER, D, exec, pkill -SIGUSR1 wayscriber");

        self.acquire_daemon_lock()?;

        // Set up signal handling
        let mut signals = Signals::new([SIGUSR1, SIGTERM, SIGINT])
            .context("Failed to register signal handler")?;

        let toggle_flag = self.toggle_requested.clone();
        let quit_flag = self.should_quit.clone();

        // Spawn signal handler thread
        // Note: This thread will run until process termination. The signal_hook iterator
        // doesn't provide a clean shutdown mechanism with forever(), but this is acceptable
        // for a daemon process as the thread has no resources requiring explicit cleanup.
        // The thread will be terminated by the OS when the process exits.
        thread::spawn(move || {
            for sig in signals.forever() {
                if quit_flag.load(Ordering::Acquire) {
                    info!("Signal handler thread exiting");
                    break;
                }
                match sig {
                    SIGUSR1 => {
                        info!("Received SIGUSR1 - toggling overlay");
                        // Use Release ordering to ensure all prior memory operations
                        // are visible to the thread that reads this flag
                        toggle_flag.store(true, Ordering::Release);
                    }
                    SIGTERM | SIGINT => {
                        info!(
                            "Received {} - initiating graceful shutdown",
                            if sig == SIGTERM { "SIGTERM" } else { "SIGINT" }
                        );
                        // Use Release ordering to ensure all prior memory operations
                        // are visible to the thread that reads this flag
                        quit_flag.store(true, Ordering::Release);
                    }
                    _ => {
                        warn!("Received unexpected signal: {}", sig);
                    }
                }
            }
        });

        // Start system tray (optional)
        if self.tray_enabled {
            let tray_toggle = self.toggle_requested.clone();
            let tray_quit = self.should_quit.clone();
            let tray_overlay_pid = self.overlay_pid.clone();
            #[cfg(feature = "tray")]
            let tray_status = self.tray_status.clone();
            #[cfg(not(feature = "tray"))]
            let tray_status = ();
            match start_system_tray(tray_toggle, tray_quit, tray_overlay_pid, tray_status) {
                Ok(tray_handle) => {
                    self.tray_thread = Some(tray_handle);
                }
                Err(err) => {
                    warn!("System tray unavailable: {}", err);
                    warn!(
                        "Continuing without system tray; use --no-tray or WAYSCRIBER_NO_TRAY=1 to silence this warning"
                    );
                }
            }
        } else {
            info!("System tray disabled; running daemon without tray");
        }

        info!("Daemon ready - waiting for toggle signal");

        // Main daemon loop
        loop {
            self.update_overlay_process_state()?;

            // Check for quit signal
            // Use Acquire ordering to ensure we see all memory operations
            // that happened before the flag was set
            if self.should_quit.load(Ordering::Acquire) {
                info!("Quit signal received - exiting daemon");
                break;
            }

            // Check for toggle request
            // Use Acquire ordering to ensure we see all memory operations
            // that happened before the flag was set
            if self.toggle_requested.swap(false, Ordering::Acquire)
                && let Err(err) = self.toggle_overlay()
            {
                warn!("Toggle overlay failed: {}", err);
            }

            // Small sleep to avoid busy-waiting
            thread::sleep(Duration::from_millis(100));
        }

        info!("Daemon shutting down");
        // Ensure overlay is stopped before exit
        if let Err(err) = self.hide_overlay() {
            warn!("Failed to hide overlay during shutdown: {}", err);
        }
        self.should_quit.store(true, Ordering::Release);
        if let Some(handle) = self.tray_thread.take() {
            match handle.join() {
                Ok(()) => info!("System tray thread joined"),
                Err(err) => warn!("System tray thread panicked: {:?}", err),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
impl Daemon {
    pub fn test_state(&self) -> OverlayState {
        self.overlay_state
    }
}
