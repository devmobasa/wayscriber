#![cfg_attr(not(feature = "tray"), allow(unused_imports))]
/// Daemon mode implementation: background service with toggle activation
use anyhow::{Context, Result, anyhow};
#[cfg(feature = "tray")]
use ksni::TrayMethods;
#[cfg(feature = "tray")]
use log::{debug, error, info, warn};
#[cfg(not(feature = "tray"))]
use log::{debug, info, warn};
#[cfg(feature = "tray")]
use png::Decoder;
use signal_hook::consts::signal::{SIGINT, SIGTERM, SIGUSR1};
use signal_hook::iterator::Signals;
use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
#[cfg(feature = "tray")]
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
#[cfg(feature = "tray")]
use std::sync::mpsc;
#[cfg(feature = "tray")]
use std::sync::{Mutex, atomic::AtomicU64};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[cfg(test)]
use crate::SESSION_OVERRIDE_FOLLOW_CONFIG;
#[cfg(feature = "tray")]
use crate::config::Config;
use crate::paths::daemon_lock_file;
use crate::session::try_lock_exclusive;
use crate::{
    RESUME_SESSION_ENV, decode_session_override, encode_session_override, runtime_session_override,
    set_runtime_session_override,
};
#[cfg(feature = "tray")]
use zbus::{Connection, Proxy};
#[cfg(feature = "tray")]
use crate::{
    paths::{log_dir, tray_action_file},
    session::{clear_session, options_from_config},
};

/// Overlay state for daemon mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,  // Daemon running, overlay not visible
    Visible, // Overlay active, capturing input
}

#[derive(Debug, Clone)]
struct OverlaySpawnErrorInfo {
    message: String,
    next_retry_at: Option<Instant>,
}

#[derive(Debug, Clone)]
struct OverlaySpawnCandidate {
    program: OsString,
    source: &'static str,
}

#[cfg(feature = "tray")]
#[derive(Debug, Default, Clone)]
struct TrayStatus {
    overlay_error: Option<OverlaySpawnErrorInfo>,
    watcher_offline: bool,
    watcher_reason: Option<String>,
}

#[cfg(feature = "tray")]
#[derive(Debug)]
struct TrayStatusShared {
    inner: Mutex<TrayStatus>,
    revision: AtomicU64,
}

#[cfg(feature = "tray")]
impl TrayStatusShared {
    fn new() -> Self {
        Self {
            inner: Mutex::new(TrayStatus::default()),
            revision: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> TrayStatus {
        self.inner.lock().unwrap().clone()
    }

    fn set_overlay_error(&self, error: Option<OverlaySpawnErrorInfo>) {
        let mut status = self.inner.lock().unwrap();
        status.overlay_error = error;
        self.bump_revision();
    }

    fn set_watcher_offline(&self, reason: String) -> bool {
        let mut status = self.inner.lock().unwrap();
        let was_offline = status.watcher_offline;
        status.watcher_offline = true;
        status.watcher_reason = Some(reason);
        self.bump_revision();
        !was_offline
    }

    fn set_watcher_online(&self) -> bool {
        let mut status = self.inner.lock().unwrap();
        let was_offline = status.watcher_offline;
        status.watcher_offline = false;
        status.watcher_reason = None;
        self.bump_revision();
        was_offline
    }

    fn bump_revision(&self) {
        self.revision.fetch_add(1, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct AlreadyRunningError;

impl std::fmt::Display for AlreadyRunningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "wayscriber daemon is already running")
    }
}

impl std::error::Error for AlreadyRunningError {}

/// Daemon state manager
type BackendRunner = dyn Fn(Option<String>) -> Result<()> + Send + Sync;

#[cfg(feature = "tray")]
const TRAY_START_TIMEOUT: Duration = Duration::from_secs(5);
const OVERLAY_SPAWN_BACKOFF_BASE: Duration = Duration::from_secs(1);
const OVERLAY_SPAWN_BACKOFF_MAX: Duration = Duration::from_secs(30);

#[cfg(feature = "tray")]
/// Tray-to-overlay IPC:
/// - Daemon writes an action string to `tray_action_file()` and signals SIGUSR2 to the overlay PID.
/// - If the overlay is not running, the daemon will auto-toggle it so queued actions run at startup.
/// - The overlay consumes the action file on SIGUSR2 and also once at startup to catch queued work.
/// - Action strings are simple identifiers (e.g., "toggle_freeze", "capture_full", "toggle_help").
///   This is intentionally simple/best-effort; if a write happens between read/delete, it will be
///   processed on the next signal/start.
#[allow(dead_code)]
const _: () = ();

#[cfg(feature = "tray")]
fn load_session_resume_enabled_from_config() -> bool {
    match Config::load() {
        Ok(loaded) => {
            let session = loaded.config.session;
            session.persist_transparent
                || session.persist_whiteboard
                || session.persist_blackboard
                || session.persist_history
                || session.restore_tool_state
        }
        Err(err) => {
            warn!(
                "Failed to read config for session resume state; assuming disabled: {}",
                err
            );
            false
        }
    }
}

#[cfg(feature = "tray")]
fn update_session_resume_in_config(target_enabled: bool, fallback: bool) -> bool {
    match Config::load() {
        Ok(loaded) => {
            let mut config = loaded.config;
            config.session.persist_transparent = target_enabled;
            config.session.persist_whiteboard = target_enabled;
            config.session.persist_blackboard = target_enabled;
            config.session.persist_history = target_enabled;
            config.session.restore_tool_state = target_enabled;
            if let Err(err) = config.save() {
                warn!(
                    "Failed to write session resume setting to config (desired {}): {}",
                    target_enabled, err
                );
                fallback
            } else {
                target_enabled
            }
        }
        Err(err) => {
            warn!(
                "Failed to load config while toggling session resume (desired {}): {}",
                target_enabled, err
            );
            fallback
        }
    }
}

pub struct Daemon {
    overlay_state: OverlayState,
    should_quit: Arc<AtomicBool>,
    toggle_requested: Arc<AtomicBool>,
    initial_mode: Option<String>,
    tray_enabled: bool,
    backend_runner: Option<Arc<BackendRunner>>,
    tray_thread: Option<JoinHandle<()>>,
    overlay_child: Option<Child>,
    overlay_pid: Arc<AtomicU32>,
    session_resume_override: Arc<AtomicU8>,
    lock_file: Option<std::fs::File>,
    overlay_spawn_failures: u32,
    overlay_spawn_next_retry: Option<Instant>,
    overlay_spawn_backoff_logged: bool,
    #[cfg(feature = "tray")]
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
pub(crate) struct WayscriberTray {
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    configurator_binary: String,
    session_resume_enabled: bool,
    overlay_pid: Arc<AtomicU32>,
    tray_action_path: PathBuf,
    tray_status: Arc<TrayStatusShared>,
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn new(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        configurator_binary: String,
        session_resume_enabled: bool,
        overlay_pid: Arc<AtomicU32>,
        tray_action_path: PathBuf,
        tray_status: Arc<TrayStatusShared>,
    ) -> Self {
        Self {
            toggle_flag,
            quit_flag,
            configurator_binary,
            session_resume_enabled,
            overlay_pid,
            tray_action_path,
            tray_status,
        }
    }

    #[cfg(test)]
    fn new_for_tests(
        toggle_flag: Arc<AtomicBool>,
        quit_flag: Arc<AtomicBool>,
        session_resume_enabled: bool,
    ) -> Self {
        Self::new(
            toggle_flag,
            quit_flag,
            "true".into(),
            session_resume_enabled,
            Arc::new(AtomicU32::new(0)),
            tray_action_file(),
            Arc::new(TrayStatusShared::new()),
        )
    }
}

#[cfg(feature = "tray")]
impl WayscriberTray {
    fn launch_configurator(&self) {
        let mut command = Command::new(&self.configurator_binary);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => {
                info!(
                    "Launched wayscriber-configurator (binary: {}, pid: {})",
                    self.configurator_binary,
                    child.id()
                );
            }
            Err(err) => {
                let not_found = err.kind() == ErrorKind::NotFound;
                if not_found {
                    error!(
                        "Configurator not found (looked for '{}'). Install 'wayscriber-configurator' (Arch: yay -S wayscriber-configurator; deb/rpm users: grab the wayscriber-configurator package from the release page) or set WAYSCRIBER_CONFIGURATOR to its path.",
                        self.configurator_binary
                    );
                } else {
                    error!(
                        "Failed to launch wayscriber-configurator using '{}': {}",
                        self.configurator_binary, err
                    );
                    error!(
                        "Set WAYSCRIBER_CONFIGURATOR to override the executable path if needed."
                    );
                }
                #[cfg(feature = "dbus")]
                {
                    let body = if not_found {
                        "Install wayscriber-configurator or set WAYSCRIBER_CONFIGURATOR to its path."
                    } else {
                        "Failed to launch configurator; see logs for details."
                    };
                    match tokio::runtime::Handle::try_current() {
                        Ok(handle) => crate::notification::send_notification_async(
                            &handle,
                            "Configurator unavailable".to_string(),
                            body.to_string(),
                            Some("dialog-error".to_string()),
                        ),
                        Err(_) => {
                            if let Ok(rt) = tokio::runtime::Runtime::new() {
                                let _ = rt.block_on(crate::notification::send_notification(
                                    "Configurator unavailable",
                                    body,
                                    Some("dialog-error"),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    fn launch_about(&self) {
        let exe = match env::current_exe() {
            Ok(path) => path,
            Err(err) => {
                warn!(
                    "Failed to resolve current executable for About window: {}",
                    err
                );
                return;
            }
        };

        let mut command = Command::new(exe);
        command
            .arg("--about")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => {
                info!("Launched About window (pid {})", child.id());
            }
            Err(err) => {
                warn!("Failed to launch About window: {}", err);
            }
        }
    }

    fn dispatch_overlay_action(&self, action: &str) {
        if let Some(parent) = self.tray_action_path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            warn!(
                "Failed to prepare tray action directory {}: {}",
                parent.display(),
                err
            );
            return;
        }

        if let Err(err) = fs::write(&self.tray_action_path, action) {
            warn!(
                "Failed to write tray action {} to {}: {}",
                action,
                self.tray_action_path.display(),
                err
            );
            return;
        }

        let pid = self.overlay_pid.load(Ordering::Acquire);

        #[cfg(unix)]
        {
            if pid != 0 {
                if unsafe { libc::kill(pid as i32, libc::SIGUSR2) } != 0 {
                    warn!(
                        "Failed to signal overlay process {} for tray action {}: {}",
                        pid,
                        action,
                        std::io::Error::last_os_error()
                    );
                }
            } else {
                // Overlay not running; request it to show so the action can run on startup.
                self.toggle_flag.store(true, Ordering::Release);
            }
        }
        #[cfg(not(unix))]
        {
            if pid == 0 {
                self.toggle_flag.store(true, Ordering::Release);
            } else {
                warn!("Tray overlay actions are only supported on Unix platforms");
            }
        }
    }

    fn clear_session_files(&self) {
        match Config::load() {
            Ok(loaded) => {
                let config_dir = match Config::config_directory_from_source(&loaded.source) {
                    Ok(dir) => dir,
                    Err(err) => {
                        warn!("Failed to resolve config directory: {}", err);
                        return;
                    }
                };
                match options_from_config(&loaded.config.session, &config_dir, None) {
                    Ok(opts) => match clear_session(&opts) {
                        Ok(outcome) => {
                            info!("Cleared session files: {:?}", outcome);
                        }
                        Err(err) => warn!("Failed to clear session files: {}", err),
                    },
                    Err(err) => warn!("Failed to build session options: {}", err),
                }
            }
            Err(err) => warn!("Failed to load config for clearing session: {}", err),
        }
    }

    fn open_log_folder(&self) {
        let dir = log_dir();
        if let Err(err) = fs::create_dir_all(&dir) {
            warn!("Failed to create log directory {}: {}", dir.display(), err);
            return;
        }

        let mut command = Command::new("xdg-open");
        command.arg(&dir);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        match command.spawn() {
            Ok(child) => info!("Opened log directory via xdg-open (pid {})", child.id()),
            Err(err) => warn!("Failed to open log directory {}: {}", dir.display(), err),
        }
    }

    fn open_config_file(&self) {
        let path = match Config::get_config_path() {
            Ok(p) => p,
            Err(err) => {
                warn!("Unable to resolve config path: {}", err);
                return;
            }
        };

        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "xdg-open"
        };

        let mut cmd = Command::new(opener);
        if cfg!(target_os = "windows") {
            cmd.args(["/C", "start", ""]).arg(&path);
        } else {
            cmd.arg(&path);
        }

        match cmd.spawn() {
            Ok(child) => info!(
                "Opened config file at {} (pid {})",
                path.display(),
                child.id()
            ),
            Err(err) => warn!("Failed to open config file at {}: {}", path.display(), err),
        }
    }

    fn tray_icon_pixmap(&self) -> Vec<ksni::Icon> {
        decode_tray_icon_png().unwrap_or_else(fallback_tray_icon)
    }
}

#[cfg(feature = "tray")]
impl ksni::Tray for WayscriberTray {
    fn id(&self) -> String {
        "wayscriber".into()
    }

    fn title(&self) -> String {
        "Wayscriber Screen Annotation".into()
    }

    fn icon_name(&self) -> String {
        "wayscriber".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let status = self.tray_status.snapshot();
        let TrayStatus {
            overlay_error,
            watcher_offline,
            watcher_reason,
        } = status;
        let mut description =
            "Toggle overlay, open configurator, or quit from the tray".to_string();

        if watcher_offline {
            description.push_str("\nTray watcher offline");
            if let Some(reason) = watcher_reason {
                description.push_str(": ");
                description.push_str(&reason);
            }
        }

        if let Some(error) = overlay_error {
            description.push_str("\nOverlay error: ");
            description.push_str(&error.message);
            if let Some(next_retry_at) = error.next_retry_at {
                let remaining = next_retry_at.saturating_duration_since(Instant::now());
                if remaining > Duration::from_secs(0) {
                    description.push_str(&format!(
                        " (retry in {}s)",
                        remaining.as_secs().max(1)
                    ));
                }
            }
        }

        ksni::ToolTip {
            icon_name: "wayscriber".into(),
            icon_pixmap: vec![],
            title: format!("Wayscriber {}", env!("CARGO_PKG_VERSION")),
            description,
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.tray_icon_pixmap()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        vec![
            StandardItem {
                label: "About Wayscriber".to_string(),
                icon_name: "help-about".into(),
                activate: Box::new(|this: &mut Self| {
                    this.launch_about();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Toggle Overlay".to_string(),
                icon_name: "tool-pointer".into(),
                activate: Box::new(|this: &mut Self| {
                    this.toggle_flag.store(true, Ordering::Release);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Toggle Freeze (overlay)".to_string(),
                icon_name: "media-playback-pause".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("toggle_freeze");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture: Full Screen".to_string(),
                icon_name: "camera-photo".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("capture_full");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture: Active Window".to_string(),
                icon_name: "window-duplicate".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("capture_window");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Capture: Region".to_string(),
                icon_name: "selection-rectangular".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("capture_region");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Toggle Help Overlay".to_string(),
                icon_name: "help-browser".into(),
                activate: Box::new(|this: &mut Self| {
                    this.dispatch_overlay_action("toggle_help");
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Configurator".to_string(),
                icon_name: "preferences-desktop".into(),
                activate: Box::new(|this: &mut Self| {
                    this.launch_configurator();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Config File".to_string(),
                icon_name: "text-x-generic".into(),
                activate: Box::new(|this: &mut Self| {
                    this.open_config_file();
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: if self.session_resume_enabled {
                    "Session resume: enabled".to_string()
                } else {
                    "Session resume: disabled".to_string()
                },
                checked: self.session_resume_enabled,
                icon_name: "document-save".into(),
                activate: Box::new(|this: &mut Self| {
                    let target = !this.session_resume_enabled;
                    let persisted =
                        update_session_resume_in_config(target, this.session_resume_enabled);
                    this.session_resume_enabled = persisted;
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Clear saved session data".to_string(),
                icon_name: "edit-clear".into(),
                activate: Box::new(|this: &mut Self| {
                    this.clear_session_files();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open log folder".to_string(),
                icon_name: "folder".into(),
                activate: Box::new(|this: &mut Self| {
                    this.open_log_folder();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "window-close".into(),
                activate: Box::new(|this: &mut Self| {
                    this.quit_flag.store(true, Ordering::Release);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn watcher_online(&self) {
        if self.tray_status.set_watcher_online() {
            info!("StatusNotifierWatcher is online");
        }
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        let reason_text = format!("{reason:?}");
        if self.tray_status.set_watcher_offline(reason_text.clone()) {
            warn!("StatusNotifierWatcher is offline: {}", reason_text);
        }
        true
    }
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

    fn session_resume_override(&self) -> Option<bool> {
        decode_session_override(self.session_resume_override.load(Ordering::Acquire))
    }

    fn apply_session_override_env(&self, command: &mut Command) {
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
        self.tray_status.set_overlay_error(Some(OverlaySpawnErrorInfo {
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
            if self.toggle_requested.swap(false, Ordering::Acquire) {
                if let Err(err) = self.toggle_overlay() {
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
        Ok(())
    }

    /// Toggle overlay visibility
    fn toggle_overlay(&mut self) -> Result<()> {
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
    fn show_overlay(&mut self) -> Result<()> {
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
    fn hide_overlay(&mut self) -> Result<()> {
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
}

#[cfg(test)]
impl Daemon {
    pub fn test_state(&self) -> OverlayState {
        self.overlay_state
    }
}

#[cfg(feature = "tray")]
fn decode_tray_icon_png() -> Option<Vec<ksni::Icon>> {
    const ICON_BYTES: &[u8] = include_bytes!("../assets/tray_icon.png");
    let decoder = Decoder::new(ICON_BYTES);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let bytes = &buf[..info.buffer_size()];
    let mut data = Vec::with_capacity(bytes.len());
    // ksni expects ARGB32 (network byte order). If the PNG is grayscale+alpha,
    // expand channels to ARGB with duplicated gray.
    match info.color_type {
        png::ColorType::Rgba => {
            for chunk in bytes.chunks_exact(4) {
                data.push(chunk[3]); // A
                data.push(chunk[0]); // R
                data.push(chunk[1]); // G
                data.push(chunk[2]); // B
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for chunk in bytes.chunks_exact(2) {
                let g = chunk[0];
                let a = chunk[1];
                data.push(a);
                data.push(g);
                data.push(g);
                data.push(g);
            }
        }
        png::ColorType::Grayscale => {
            for &g in bytes {
                data.push(255);
                data.push(g);
                data.push(g);
                data.push(g);
            }
        }
        png::ColorType::Rgb => {
            for chunk in bytes.chunks_exact(3) {
                data.push(255);
                data.push(chunk[0]);
                data.push(chunk[1]);
                data.push(chunk[2]);
            }
        }
        _ => {
            warn!("Unsupported tray icon color type; falling back to empty icon");
            return None;
        }
    }
    Some(vec![ksni::Icon {
        width: info.width as i32,
        height: info.height as i32,
        data,
    }])
}

#[cfg(feature = "tray")]
fn fallback_tray_icon() -> Vec<ksni::Icon> {
    let size = 22;
    let mut data = Vec::with_capacity(size * size * 4);

    for y in 0..size {
        for x in 0..size {
            let (a, r, g, b) = if (2..=4).contains(&x) && (2..=4).contains(&y) {
                (255, 60, 60, 60)
            } else if (3..=5).contains(&x) && (5..=7).contains(&y) {
                (255, 180, 120, 60)
            } else if (4..=8).contains(&x) && (6..=14).contains(&y) {
                (255, 255, 220, 0)
            } else if (7..=9).contains(&x) && (13..=17).contains(&y) {
                (255, 180, 180, 180)
            } else if (8..=11).contains(&x) && (16..=19).contains(&y) {
                (255, 255, 150, 180)
            } else {
                (0, 0, 0, 0)
            };

            data.push(a);
            data.push(r);
            data.push(g);
            data.push(b);
        }
    }

    vec![ksni::Icon {
        width: size as i32,
        height: size as i32,
        data,
    }]
}

/// System tray implementation
#[cfg(feature = "tray")]
fn start_system_tray(
    toggle_flag: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
    overlay_pid: Arc<AtomicU32>,
    tray_status: Arc<TrayStatusShared>,
) -> Result<JoinHandle<()>> {
    let configurator_binary = std::env::var("WAYSCRIBER_CONFIGURATOR")
        .unwrap_or_else(|_| "wayscriber-configurator".to_string());
    let session_resume_enabled = load_session_resume_enabled_from_config();

    let tray_quit_flag = quit_flag.clone();
    let tray = WayscriberTray::new(
        toggle_flag,
        tray_quit_flag.clone(),
        configurator_binary,
        session_resume_enabled,
        overlay_pid,
        tray_action_file(),
        tray_status.clone(),
    );
    let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

    info!("Creating tray service...");
    info!("Spawning system tray runtime thread...");

    let ready_thread_tx = ready_tx.clone();
    let tray_thread = thread::spawn(move || {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(e) => {
                warn!("Failed to create Tokio runtime for system tray: {}", e);
                report_tray_readiness(
                    &ready_thread_tx,
                    Err(anyhow!(
                        "Failed to create Tokio runtime for system tray: {e}"
                    )),
                );
                return;
            }
        };

        rt.block_on(async {
            match tray.assume_sni_available(true).spawn().await {
                Ok(handle) => {
                    info!("System tray spawned successfully");
                    report_tray_readiness(&ready_thread_tx, Ok(()));
                    tokio::spawn(log_status_notifier_state());
                    let mut last_revision = tray_status.revision.load(Ordering::Acquire);

                    // Monitor quit flag and shutdown gracefully
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        let revision = tray_status.revision.load(Ordering::Acquire);
                        if revision != last_revision {
                            if handle.update(|_| {}).await.is_none() {
                                warn!("Tray service closed; stopping tray monitor");
                                break;
                            }
                            last_revision = revision;
                        }
                        if tray_quit_flag.load(Ordering::Acquire) {
                            info!("Quit signal received - shutting down system tray");
                            let _ = handle.shutdown().await;
                            break;
                        }
                    }
                }
                Err(e) => {
                    warn!("System tray error: {}", e);
                    report_tray_readiness(&ready_thread_tx, Err(anyhow!("System tray error: {e}")));
                }
            }
        });
    });

    drop(ready_tx);

    info!("Waiting for system tray readiness signal...");
    match ready_rx.recv_timeout(TRAY_START_TIMEOUT) {
        Ok(result) => {
            result?;
            info!("System tray thread started");
            Ok(tray_thread)
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            warn!("Timed out waiting for system tray to start");
            quit_flag.store(true, Ordering::Release);
            let _ = tray_thread.join();
            Err(anyhow!("Timed out waiting for system tray to start"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = tray_thread.join();
            Err(anyhow!(
                "System tray thread exited before signaling readiness"
            ))
        }
    }
}

#[cfg(not(feature = "tray"))]
fn start_system_tray(
    _toggle_flag: Arc<AtomicBool>,
    _quit_flag: Arc<AtomicBool>,
    _overlay_pid: Arc<AtomicU32>,
    _tray_status: (),
) -> Result<JoinHandle<()>> {
    info!("Tray feature disabled; skipping system tray startup");
    Ok(thread::spawn(|| ()))
}

impl Daemon {
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

        warn!(
            "Overlay spawn attempts failed: {}",
            failures.join("; ")
        );
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

    fn update_overlay_process_state(&mut self) -> Result<()> {
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

#[cfg(feature = "tray")]
fn report_tray_readiness(tx: &mpsc::Sender<Result<()>>, result: Result<()>) {
    if let Err(err) = tx.send(result) {
        debug!(
            "System tray readiness receiver dropped before signal could be delivered: {}",
            err
        );
    }
}

#[cfg(feature = "tray")]
async fn log_status_notifier_state() {
    let conn = match Connection::session().await {
        Ok(conn) => conn,
        Err(err) => {
            warn!("Failed to connect to session D-Bus for tray diagnostics: {}", err);
            return;
        }
    };

    let proxy = match Proxy::new(
        &conn,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    )
    .await
    {
        Ok(proxy) => proxy,
        Err(err) => {
            warn!(
                "StatusNotifierWatcher unavailable (no tray host?): {}",
                err
            );
            return;
        }
    };

    let host_registered: bool = match proxy.get_property("IsStatusNotifierHostRegistered").await {
        Ok(value) => value,
        Err(err) => {
            warn!("Failed to query tray host registration: {}", err);
            return;
        }
    };

    let items: Vec<String> = match proxy.get_property("RegisteredStatusNotifierItems").await {
        Ok(value) => value,
        Err(err) => {
            warn!("Failed to query registered tray items: {}", err);
            return;
        }
    };

    info!(
        "StatusNotifierWatcher ready: host_registered={}, registered_items={}",
        host_registered,
        items.len()
    );
}

#[cfg(all(test, feature = "tray"))]
mod tests {
    use super::*;
    use ksni::{Tray, menu::MenuItem};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    fn runner_counter(count: Arc<AtomicUsize>) -> Arc<BackendRunner> {
        Arc::new(move |mode: Option<String>| -> Result<()> {
            assert_eq!(mode.as_deref(), Some("whiteboard"));
            count.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        })
    }

    #[test]
    fn toggle_overlay_invokes_backend_when_hidden() {
        let counter = Arc::new(AtomicUsize::new(0));
        let runner = runner_counter(counter.clone());
        let mut daemon = Daemon::with_backend_runner(Some("whiteboard".into()), runner);

        daemon.toggle_overlay().unwrap();
        assert_eq!(counter.load(AtomicOrdering::SeqCst), 1);
        assert_eq!(daemon.test_state(), OverlayState::Hidden);
    }

    #[test]
    fn hide_overlay_is_idempotent() {
        let runner = Arc::new(|_: Option<String>| Ok(())) as Arc<BackendRunner>;
        let mut daemon = Daemon::with_backend_runner(None, runner);
        daemon.hide_overlay().unwrap();
        assert_eq!(daemon.test_state(), OverlayState::Hidden);

        daemon.overlay_state = OverlayState::Visible;
        daemon.toggle_overlay().unwrap();
        assert_eq!(daemon.test_state(), OverlayState::Hidden);
    }

    fn activate_menu_item(tray: &mut WayscriberTray, label: &str) {
        for item in tray.menu() {
            match item {
                MenuItem::Standard(standard) if standard.label.contains(label) => {
                    let activate = standard.activate;
                    activate(tray);
                    return;
                }
                MenuItem::Checkmark(check) if check.label.contains(label) => {
                    let activate = check.activate;
                    activate(tray);
                    return;
                }
                _ => {}
            }
        }
        panic!("Menu item '{label}' not found");
    }

    #[test]
    fn tray_toggle_action_sets_flag() {
        let toggle = Arc::new(AtomicBool::new(false));
        let quit = Arc::new(AtomicBool::new(false));
        let mut tray = WayscriberTray::new_for_tests(toggle.clone(), quit, false);

        activate_menu_item(&mut tray, "Toggle Overlay");
        assert!(toggle.load(Ordering::SeqCst));
    }

    #[test]
    fn tray_quit_action_sets_quit_flag() {
        let toggle = Arc::new(AtomicBool::new(false));
        let quit = Arc::new(AtomicBool::new(false));
        let mut tray = WayscriberTray::new_for_tests(toggle, quit.clone(), false);

        activate_menu_item(&mut tray, "Quit");
        assert!(quit.load(Ordering::SeqCst));
    }
}
