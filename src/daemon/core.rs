use anyhow::{Context, Result};
use log::{info, warn};
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(test)]
use crate::SESSION_OVERRIDE_FOLLOW_CONFIG;
use crate::env_vars::NO_TRAY_ENV;
use crate::paths::daemon_lock_file;
use crate::session::try_lock_exclusive;
use crate::tray_action::TrayAction;
use crate::{RESUME_SESSION_ENV, decode_session_override, encode_session_override};
use wayscriber::shortcut_hint::{ShortcutRuntimeBackend, current_shortcut_runtime_backend};

#[cfg(test)]
use super::control::read_daemon_toggle_response;
use super::control::{
    DaemonToggleCommand, DaemonToggleCommands, DaemonToggleRequest,
    write_daemon_toggle_command_error, write_daemon_toggle_command_success,
};
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

    fn ensure_visible_overlay_can_accept_request(
        &self,
        request: Option<&DaemonToggleRequest>,
    ) -> Result<()> {
        let Some(requested) = request.and_then(|request| request.session_file.as_ref()) else {
            return Ok(());
        };
        if self.overlay_state != OverlayState::Visible {
            return Ok(());
        }
        if self
            .active_named_session_file
            .as_ref()
            .is_some_and(|active| named_session_paths_match(active, requested))
        {
            return Ok(());
        }

        Err(anyhow::anyhow!(
            "cannot switch named session target while overlay is visible; hide the overlay first"
        ))
    }

    fn process_single_toggle(
        &mut self,
        request: Option<DaemonToggleRequest>,
        activation_token: Option<String>,
        suppress_overlay_action_signal: bool,
    ) -> Result<bool> {
        let request = request
            .map(|mut request| {
                request.normalize_and_validate_session_file()?;
                Ok::<_, anyhow::Error>(request)
            })
            .transpose()?;
        self.ensure_visible_overlay_can_accept_request(request.as_ref())?;
        if let Some(action) = request.as_ref().and_then(|request| request.overlay_action) {
            self.pending_activation_token = activation_token;
            self.pending_toggle_request = request.filter(|request| !request.is_empty());
            if self.overlay_state == OverlayState::Hidden
                && matches!(action, TrayAction::LightDrawOff)
            {
                self.pending_activation_token = None;
                self.pending_toggle_request = None;
                return Ok(false);
            }
            let was_hidden = self.overlay_state == OverlayState::Hidden;
            self.dispatch_overlay_action(action, !suppress_overlay_action_signal)?;
            if self.overlay_state == OverlayState::Hidden {
                self.show_overlay()?;
                return Ok(was_hidden);
            } else {
                self.pending_activation_token = None;
                self.pending_toggle_request = None;
            }
            return Ok(false);
        }

        self.pending_activation_token = activation_token;
        self.pending_toggle_request = request.filter(|request| !request.is_empty());
        if let Err(err) = self.toggle_overlay() {
            self.pending_activation_token = None;
            self.pending_toggle_request = None;
            return Err(err);
        }
        Ok(false)
    }

    fn process_queued_toggle_command(
        &mut self,
        command: DaemonToggleCommand,
        suppress_overlay_action_signal: &mut bool,
    ) {
        let result = self.process_single_toggle(
            Some(command.request.clone()),
            None,
            *suppress_overlay_action_signal,
        );
        match result {
            Ok(spawned_overlay) => {
                *suppress_overlay_action_signal |= spawned_overlay;
                if let Err(err) = write_daemon_toggle_command_success(&command) {
                    warn!("Failed to write daemon toggle response: {}", err);
                }
            }
            Err(err) => {
                let message = format!("{err:#}");
                warn!("Toggle overlay failed: {}", message);
                if let Err(response_err) = write_daemon_toggle_command_error(&command, &message) {
                    warn!(
                        "Failed to write daemon toggle error response: {}",
                        response_err
                    );
                }
            }
        }
    }

    fn dispatch_overlay_action(
        &self,
        action: TrayAction,
        signal_visible_overlay: bool,
    ) -> Result<()> {
        let action_path = crate::tray_action::queue_action(action)?;

        let pid = self.overlay_pid.load(Ordering::Acquire);
        if signal_visible_overlay && self.overlay_state == OverlayState::Visible && pid != 0 {
            #[cfg(unix)]
            {
                let pid = i32::try_from(pid).context("overlay pid does not fit into i32")?;
                if unsafe { libc::kill(pid, libc::SIGUSR2) } != 0 {
                    warn!(
                        "Failed to signal overlay process {} for action {}: {}",
                        pid,
                        action.as_str(),
                        std::io::Error::last_os_error()
                    );
                }
            }
            #[cfg(not(unix))]
            {
                warn!("Overlay actions are only supported on Unix platforms");
            }
        }
        log::debug!(
            "Queued overlay action {} at {}",
            action.as_str(),
            action_path.display()
        );

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
            DaemonToggleCommands {
                commands: Vec::new(),
                saw_command_files: false,
            }
        };

        if !signal_toggle_requested || activation_token.is_some() {
            self.process_single_toggle(None, activation_token, false)?;
        }

        if signal_toggle_requested {
            self.process_signal_toggle_commands(queued_requests)?;
        }

        Ok(())
    }

    fn process_signal_toggle_commands(
        &mut self,
        queued_requests: DaemonToggleCommands,
    ) -> Result<()> {
        if queued_requests.commands.is_empty() {
            if queued_requests.saw_command_files {
                return Ok(());
            }
            return self
                .process_single_toggle(Some(DaemonToggleRequest::default()), None, false)
                .map(drop);
        }

        let mut suppress_overlay_action_signal = false;
        for command in queued_requests.commands {
            self.process_queued_toggle_command(command, &mut suppress_overlay_action_signal);
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

        // The signal listener thread runs until process termination. The daemon
        // exits shortly after quit signals, and the OS cleans up the detached thread.
        #[cfg(unix)]
        crate::unix_signals::spawn_listener(&DAEMON_SIGNALS, move |sig| {
            if quit_flag.load(Ordering::Acquire) {
                info!("Signal handler thread exiting");
                return;
            }
            match sig {
                libc::SIGUSR1 => {
                    info!("Received SIGUSR1 - toggling overlay");
                    // Use Release ordering to ensure all prior memory operations
                    // are visible to the thread that reads this flag
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
                    // Use Release ordering to ensure all prior memory operations
                    // are visible to the thread that reads this flag
                    quit_flag.store(true, Ordering::Release);
                }
                _ => {
                    warn!("Received unexpected signal: {}", sig);
                }
            }
        })
        .context("Failed to register signal handler")?;

        // Only publish the pid after SIGUSR1 is handled. A racing
        // `--daemon-toggle` sends SIGUSR1 to this pid, and the default action
        // before handler installation would terminate the daemon.
        crate::daemon::write_daemon_pid_file(std::process::id(), &self.instance_token)?;

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

fn named_session_paths_match(left: &Path, right: &Path) -> bool {
    crate::session::catalog::session_paths_match(left, right)
}

#[cfg(test)]
impl Daemon {
    pub fn test_state(&self) -> OverlayState {
        self.overlay_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    #[test]
    fn light_draw_off_request_does_not_show_hidden_overlay() {
        let called = Arc::new(AtomicUsize::new(0));
        let called_clone = Arc::clone(&called);
        let runner: Arc<BackendRunner> = Arc::new(move |_| {
            called_clone.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        });
        let mut daemon = Daemon::with_backend_runner(None, runner);

        daemon
            .process_single_toggle(
                Some(DaemonToggleRequest {
                    overlay_action: Some(TrayAction::LightDrawOff),
                    ..Default::default()
                }),
                None,
                false,
            )
            .unwrap();

        assert_eq!(called.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(daemon.test_state(), OverlayState::Hidden);
        assert!(daemon.pending_toggle_request.is_none());
        assert!(daemon.pending_activation_token.is_none());
    }

    #[test]
    fn visible_overlay_rejects_different_named_session_request() {
        let runner: Arc<BackendRunner> = Arc::new(|_| Ok(()));
        let mut daemon = Daemon::with_backend_runner(None, runner);
        daemon.overlay_state = OverlayState::Visible;
        daemon.active_named_session_file =
            Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));

        let err = daemon
            .process_single_toggle(
                Some(DaemonToggleRequest {
                    session_file: Some(std::path::PathBuf::from("/tmp/other.wayscriber-session")),
                    ..Default::default()
                }),
                None,
                false,
            )
            .expect_err("different visible named target should be rejected");

        assert!(
            format!("{err:#}")
                .contains("cannot switch named session target while overlay is visible"),
            "{err:#}"
        );
        assert_eq!(daemon.test_state(), OverlayState::Visible);
        assert_eq!(
            daemon.active_named_session_file.as_deref(),
            Some(std::path::Path::new("/tmp/current.wayscriber-session"))
        );
    }

    #[test]
    fn visible_overlay_rejection_writes_daemon_toggle_error_response() {
        let temp = crate::test_temp::tempdir().expect("tempdir");
        let runner: Arc<BackendRunner> = Arc::new(|_| Ok(()));
        let mut daemon = Daemon::with_backend_runner(None, runner);
        daemon.overlay_state = OverlayState::Visible;
        daemon.active_named_session_file =
            Some(std::path::PathBuf::from("/tmp/current.wayscriber-session"));
        let command = DaemonToggleCommand {
            daemon_token: "daemon-token".into(),
            request: DaemonToggleRequest {
                session_file: Some(std::path::PathBuf::from("/tmp/other.wayscriber-session")),
                ..Default::default()
            },
            request_path: temp.path().join("request.json"),
            response_path: temp.path().join("responses").join("request.json"),
        };

        let mut suppress_overlay_action_signal = false;
        daemon.process_queued_toggle_command(command.clone(), &mut suppress_overlay_action_signal);

        let err = read_daemon_toggle_response(&command.response_path)
            .expect_err("visible target mismatch should be written to response");
        assert!(
            format!("{err:#}")
                .contains("cannot switch named session target while overlay is visible"),
            "{err:#}"
        );
        assert_eq!(daemon.test_state(), OverlayState::Visible);
        assert!(!suppress_overlay_action_signal);
    }

    #[test]
    fn typed_signal_with_no_executable_commands_does_not_fallback_to_raw_toggle() {
        let called = Arc::new(AtomicUsize::new(0));
        let called_clone = Arc::clone(&called);
        let runner: Arc<BackendRunner> = Arc::new(move |_| {
            called_clone.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        });
        let mut daemon = Daemon::with_backend_runner(None, runner);

        daemon
            .process_signal_toggle_commands(DaemonToggleCommands {
                commands: Vec::new(),
                saw_command_files: true,
            })
            .expect("typed command marker should suppress raw fallback");

        assert_eq!(called.load(AtomicOrdering::SeqCst), 0);
        assert_eq!(daemon.test_state(), OverlayState::Hidden);
    }
}
