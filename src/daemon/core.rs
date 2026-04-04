use anyhow::{Context, Result};
use log::{info, warn};
use signal_hook::consts::signal::{SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(test)]
use crate::SESSION_OVERRIDE_FOLLOW_CONFIG;
use crate::paths::daemon_lock_file;
use crate::session::try_lock_exclusive;
use crate::{RESUME_SESSION_ENV, decode_session_override, encode_session_override};
use wayscriber::shortcut_hint::{ShortcutRuntimeBackend, current_shortcut_runtime_backend};

use super::control::DaemonToggleRequest;
use super::global_shortcuts::start_global_shortcuts_listener;
use super::tray::start_system_tray;
#[cfg(feature = "tray")]
use super::types::TrayStatusShared;
use super::types::{AlreadyRunningError, BackendRunner, OverlayState};

pub struct Daemon {
    pub(super) overlay_state: OverlayState,
    pub(super) should_quit: Arc<AtomicBool>,
    pub(super) toggle_requested: Arc<AtomicBool>,
    pub(super) signal_toggle_requested: Arc<AtomicBool>,
    pub(super) initial_mode: Option<String>,
    pub(super) instance_token: String,
    pub(super) freeze_on_show: bool,
    pub(super) tray_enabled: bool,
    pub(super) backend_runner: Option<Arc<BackendRunner>>,
    pub(super) tray_thread: Option<JoinHandle<()>>,
    pub(super) global_shortcuts_thread: Option<JoinHandle<()>>,
    pub(super) overlay_child: Option<Child>,
    pub(super) overlay_pid: Arc<AtomicU32>,
    pub(super) pending_activation_token: Option<String>,
    pub(super) pending_toggle_request: Option<DaemonToggleRequest>,
    pub(super) portal_activation_token_slot: Arc<Mutex<Option<String>>>,
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
            signal_toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
            instance_token: crate::daemon::generate_daemon_instance_token(),
            freeze_on_show: false,
            tray_enabled,
            backend_runner: None,
            tray_thread: None,
            global_shortcuts_thread: None,
            overlay_child: None,
            overlay_pid: Arc::new(AtomicU32::new(0)),
            pending_activation_token: None,
            pending_toggle_request: None,
            portal_activation_token_slot: Arc::new(Mutex::new(None)),
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
            signal_toggle_requested: Arc::new(AtomicBool::new(false)),
            initial_mode,
            instance_token: crate::daemon::generate_daemon_instance_token(),
            freeze_on_show: false,
            tray_enabled: true,
            backend_runner: Some(backend_runner),
            tray_thread: None,
            global_shortcuts_thread: None,
            overlay_child: None,
            overlay_pid: Arc::new(AtomicU32::new(0)),
            pending_activation_token: None,
            pending_toggle_request: None,
            portal_activation_token_slot: Arc::new(Mutex::new(None)),
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

    pub fn set_freeze_on_show(&mut self, enabled: bool) {
        self.freeze_on_show = enabled;
    }

    fn process_single_toggle(
        &mut self,
        request: Option<DaemonToggleRequest>,
        activation_token: Option<String>,
    ) -> Result<()> {
        self.pending_activation_token = activation_token;
        self.pending_toggle_request = request.filter(|request| !request.is_empty());
        if let Err(err) = self.toggle_overlay() {
            self.pending_activation_token = None;
            self.pending_toggle_request = None;
            return Err(err);
        }
        Ok(())
    }

    fn process_pending_toggles(
        &mut self,
        activation_token: Option<String>,
        signal_toggle_requested: bool,
    ) -> Result<()> {
        let queued_requests = if signal_toggle_requested {
            crate::daemon::take_daemon_toggle_requests(&self.instance_token)?
        } else {
            Vec::new()
        };

        if !signal_toggle_requested || activation_token.is_some() {
            self.process_single_toggle(None, activation_token)?;
        }

        if signal_toggle_requested {
            let requests = if queued_requests.is_empty() {
                vec![DaemonToggleRequest::default()]
            } else {
                queued_requests
            };

            for request in requests {
                self.process_single_toggle(Some(request), None)?;
            }
        }

        Ok(())
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
        if self.freeze_on_show {
            info!("Daemon activations will request frozen mode on show");
        }
        info!("Daemon control command: wayscriber --daemon-toggle [--freeze] [--mode …]");
        info!("Preferred external control: wayscriber --daemon-toggle");
        info!("Legacy raw SIGUSR1 toggle still works, but cannot carry launch args");

        self.acquire_daemon_lock()?;
        if let Err(err) = crate::daemon::clear_daemon_toggle_request_file() {
            warn!(
                "Failed to clear stale daemon toggle request on startup: {}",
                err
            );
        }

        // Set up signal handling
        let mut signals = Signals::new([SIGUSR1, SIGTERM, SIGINT])
            .context("Failed to register signal handler")?;

        crate::daemon::write_daemon_pid_file(std::process::id(), &self.instance_token)?;

        let toggle_flag = self.toggle_requested.clone();
        let signal_toggle_flag = self.signal_toggle_requested.clone();
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
                        signal_toggle_flag.store(true, Ordering::Release);
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

        match current_shortcut_runtime_backend() {
            ShortcutRuntimeBackend::PortalGlobalShortcuts => {
                self.global_shortcuts_thread = start_global_shortcuts_listener(
                    self.toggle_requested.clone(),
                    self.should_quit.clone(),
                    self.portal_activation_token_slot.clone(),
                );
                if self.global_shortcuts_thread.is_some() {
                    info!("Global shortcuts portal listener started");
                }
            }
            ShortcutRuntimeBackend::GnomeCustomShortcut => {
                info!(
                    "Global shortcuts portal listener skipped on GNOME; using GNOME shortcut backend"
                );
            }
            ShortcutRuntimeBackend::Manual => {
                info!("Global shortcuts portal listener skipped; portal runtime unavailable");
            }
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
            if self.toggle_requested.swap(false, Ordering::Acquire) {
                let signal_toggle_requested =
                    self.signal_toggle_requested.swap(false, Ordering::Acquire);
                let pending_token = self
                    .portal_activation_token_slot
                    .lock()
                    .unwrap_or_else(|poisoned| {
                        warn!("portal activation token mutex poisoned; recovering");
                        poisoned.into_inner()
                    })
                    .take();
                if let Err(err) =
                    self.process_pending_toggles(pending_token, signal_toggle_requested)
                {
                    warn!("Toggle overlay failed: {}", err);
                }
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
        if let Some(handle) = self.global_shortcuts_thread.take() {
            match handle.join() {
                Ok(()) => info!("Global shortcuts listener thread joined"),
                Err(err) => warn!("Global shortcuts listener thread panicked: {:?}", err),
            }
        }
        if let Err(err) = crate::daemon::clear_daemon_toggle_request_file() {
            warn!("Failed to clear daemon toggle request file: {}", err);
        }
        if let Err(err) = crate::daemon::clear_daemon_pid_file() {
            warn!("Failed to clear daemon pid file: {}", err);
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
