use anyhow::{Context, Result};
use log::{info, warn};
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
#[cfg(test)]
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::backend::wayland::RuntimeWakeSource;
use crate::env_vars::NO_TRAY_ENV;
use crate::paths::daemon_lock_file;
use crate::session::try_lock_exclusive;
#[cfg(test)]
use crate::session_override::SESSION_OVERRIDE_FOLLOW_CONFIG;
use crate::shortcut_hint::{ShortcutRuntimeBackend, current_shortcut_runtime_backend};
#[cfg(test)]
use crate::tray_action::TrayAction;
use crate::{RESUME_SESSION_ENV, decode_session_override, encode_session_override};

use super::control::DaemonToggleRequest;
#[cfg(test)]
use super::control::read_daemon_toggle_response;
#[cfg(test)]
use super::control::{DaemonToggleCommand, DaemonToggleCommands};
use super::global_shortcuts::start_global_shortcuts_listener;
use super::tray::start_system_tray;
#[cfg(feature = "tray")]
use super::types::TrayStatusShared;
use super::types::{AlreadyRunningError, BackendRunner, OverlayState};

// Some desktop custom shortcut runners, observed on KDE, can launch the same
// plain `--daemon-toggle` command twice about 400-600ms apart from one key press.
// Suppress only duplicate plain toggles after a successful toggle completes, so
// typed requests still run.
const DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW: Duration = Duration::from_millis(700);
// This remains a real child/process/tray lifecycle deadline. Signal delivery
// and signal-listener failure independently wake the daemon control owner.
const DAEMON_LIFECYCLE_POLL_INTERVAL: Duration = Duration::from_millis(100);

mod toggles;

pub struct Daemon {
    pub(super) overlay_state: OverlayState,
    pub(super) should_quit: Arc<AtomicBool>,
    pub(super) toggle_requested: Arc<AtomicBool>,
    pub(super) signal_toggle_requested: Arc<AtomicBool>,
    pub(super) initial_mode: Option<String>,
    pub(super) initial_named_session_file: Option<PathBuf>,
    pub(super) active_named_session_file: Option<PathBuf>,
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
    pub(super) last_plain_visibility_toggle_completed_at: Option<Instant>,
    #[cfg(unix)]
    signal_listener: Option<crate::unix_signals::SignalListener>,
    #[cfg(feature = "tray")]
    pub(super) tray_status: Arc<TrayStatusShared>,
}

impl Daemon {
    pub fn new(
        initial_mode: Option<String>,
        tray_enabled: bool,
        session_resume_override: Option<bool>,
        initial_named_session_file: Option<PathBuf>,
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
            initial_named_session_file,
            active_named_session_file: None,
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
            last_plain_visibility_toggle_completed_at: None,
            #[cfg(unix)]
            signal_listener: None,
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
            initial_named_session_file: None,
            active_named_session_file: None,
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
            last_plain_visibility_toggle_completed_at: None,
            #[cfg(unix)]
            signal_listener: None,
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

    pub(super) fn effective_named_session_file(&self) -> Option<PathBuf> {
        self.pending_toggle_request
            .as_ref()
            .and_then(|request| request.session_file.clone())
            .or_else(|| self.initial_named_session_file.clone())
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
        if let Err(err) = crate::daemon::clear_daemon_pid_file() {
            warn!("Failed to clear stale daemon pid file on startup: {}", err);
        }
        if let Err(err) = crate::daemon::clear_daemon_toggle_request_file() {
            warn!(
                "Failed to clear stale daemon toggle request on startup: {}",
                err
            );
        }

        #[cfg(unix)]
        const DAEMON_SIGNALS: [libc::c_int; 3] = [libc::SIGUSR1, libc::SIGTERM, libc::SIGINT];

        let toggle_flag = self.toggle_requested.clone();
        let signal_toggle_flag = self.signal_toggle_requested.clone();
        let quit_flag = self.should_quit.clone();

        let daemon_wake =
            RuntimeWakeSource::new().context("Failed to create daemon control wake descriptor")?;

        #[cfg(unix)]
        {
            let listener_wake = daemon_wake.handle();
            self.signal_listener = Some(
                crate::unix_signals::spawn_listener(
                    &DAEMON_SIGNALS,
                    move |sig| {
                        if quit_flag.load(Ordering::Acquire) {
                            return;
                        }
                        match sig {
                            libc::SIGUSR1 => {
                                info!("Received SIGUSR1 - toggling overlay");
                                signal_toggle_flag.store(true, Ordering::Release);
                                toggle_flag.store(true, Ordering::Release);
                            }
                            libc::SIGTERM | libc::SIGINT => {
                                info!(
                                    "Received {} - initiating graceful shutdown",
                                    if sig == libc::SIGTERM {
                                        "SIGTERM"
                                    } else {
                                        "SIGINT"
                                    }
                                );
                                quit_flag.store(true, Ordering::Release);
                            }
                            _ => warn!("Received unexpected signal: {sig}"),
                        }
                    },
                    move || {
                        if let Err(err) = listener_wake.wake() {
                            warn!("Failed to wake daemon after signal publication: {err}");
                        }
                    },
                )
                .context("Failed to register signal handler")?,
            );
        }

        // Only publish the pid after SIGUSR1 is handled. A racing
        // `--daemon-toggle` sends SIGUSR1 to this pid, and the default action
        // before handler installation would terminate the daemon.
        if let Err(err) =
            crate::daemon::write_daemon_pid_file(std::process::id(), &self.instance_token)
        {
            if let Err(stop_err) = self.stop_signal_listener() {
                warn!(
                    "Failed to stop signal listener after readiness publication error: {stop_err}"
                );
            }
            return Err(err);
        }

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
                        "Continuing without system tray; use --no-tray or {NO_TRAY_ENV}=1 to silence this warning"
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

        let run_result = self.run_control_loop_and_invalidate_on_failure(&daemon_wake);
        let cleanup_result = self.shutdown_after_run();
        run_result.and(cleanup_result)
    }

    fn run_control_loop_and_invalidate_on_failure(
        &mut self,
        daemon_wake: &RuntimeWakeSource,
    ) -> Result<()> {
        let result = self.run_control_loop(daemon_wake);
        if result.is_err()
            && let Err(err) = crate::daemon::clear_daemon_pid_file()
        {
            warn!("Failed to invalidate daemon readiness after runtime failure: {err}");
        }
        result
    }

    fn run_control_loop(&mut self, daemon_wake: &RuntimeWakeSource) -> Result<()> {
        loop {
            self.ensure_signal_listener_healthy()?;
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

            wait_for_daemon_lifecycle(daemon_wake)?;
        }
        Ok(())
    }

    fn shutdown_after_run(&mut self) -> Result<()> {
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
        self.stop_signal_listener()
    }

    fn ensure_signal_listener_healthy(&self) -> Result<()> {
        #[cfg(unix)]
        {
            let listener = self
                .signal_listener
                .as_ref()
                .context("daemon signal listener is not installed")?;
            match listener.health() {
                crate::unix_signals::SignalListenerHealth::Running => Ok(()),
                crate::unix_signals::SignalListenerHealth::Failed(failure) => {
                    Err(anyhow::anyhow!("daemon signal listener failed: {failure}"))
                }
                health => Err(anyhow::anyhow!(
                    "daemon signal listener stopped unexpectedly: {health:?}"
                )),
            }
        }

        #[cfg(not(unix))]
        {
            Ok(())
        }
    }

    fn stop_signal_listener(&mut self) -> Result<()> {
        #[cfg(unix)]
        if let Some(mut listener) = self.signal_listener.take() {
            let failure = match listener.health() {
                crate::unix_signals::SignalListenerHealth::Failed(failure) => Some(failure),
                _ => None,
            };
            listener
                .stop_and_join()
                .context("failed to stop daemon signal listener")?;
            if let Some(failure) = failure {
                return Err(anyhow::anyhow!(
                    "daemon signal listener failed before teardown: {failure}"
                ));
            }
        }
        Ok(())
    }
}

fn wait_for_daemon_lifecycle(daemon_wake: &RuntimeWakeSource) -> Result<()> {
    let mut pollfd = libc::pollfd {
        fd: daemon_wake.poll_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let deadline = Instant::now() + DAEMON_LIFECYCLE_POLL_INTERVAL;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout_ms = remaining.as_millis().min(i32::MAX as u128) as i32;
        // SAFETY: the descriptor remains owned by `daemon_wake` throughout poll.
        let ready = unsafe { libc::poll(&mut pollfd, 1, timeout_ms) };
        if ready == 0 {
            return Ok(());
        }
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == ErrorKind::Interrupted {
                continue;
            }
            return Err(err).context("daemon lifecycle poll failed");
        }
        let terminal = pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL);
        if terminal != 0 || pollfd.revents & libc::POLLIN == 0 {
            return Err(anyhow::anyhow!(
                "daemon wake descriptor returned invalid readiness {:#x}",
                pollfd.revents
            ));
        }
        daemon_wake
            .drain()
            .context("failed to drain daemon wake descriptor")?;
        return Ok(());
    }
}

#[cfg(test)]
impl Daemon {
    pub fn test_state(&self) -> OverlayState {
        self.overlay_state
    }
}

#[cfg(test)]
mod tests;
