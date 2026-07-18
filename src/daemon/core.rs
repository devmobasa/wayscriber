use anyhow::{Context, Result};
use log::{info, warn};
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::backend::wayland::RuntimeWakeSource;
use crate::env_vars::NO_TRAY_ENV;
use crate::paths::daemon_lock_file;
use crate::session::try_lock_exclusive;
#[cfg(test)]
use crate::session_override::SESSION_OVERRIDE_FOLLOW_CONFIG;
use crate::shortcut_hint::{ShortcutRuntimeBackend, current_shortcut_runtime_backend};
use crate::tray_action::TrayAction;
use crate::{decode_session_override, encode_session_override};

use super::control::DaemonToggleRequest;
#[cfg(test)]
use super::control::read_daemon_toggle_response;
#[cfg(test)]
use super::control::{DaemonToggleCommand, DaemonToggleCommands};
use super::global_shortcuts::{GlobalShortcutsListener, start_global_shortcuts_listener};
use super::protocol_v2::DaemonControlProtocolMode;
use super::protocol_v2::OverlayChildOwner;
use super::protocol_v2::{
    ActionJournal, BootClock, BootDeadline, BootDeadlineSource, CommandOwner, CommandQueueWatcher,
    DaemonRuntimeRecordV2, EffectKind, FinalEffect, ProtocolToken,
};
use super::tray::start_system_tray;
#[cfg(feature = "tray")]
use super::types::TrayStatusShared;
use super::types::{
    AlreadyRunningError, BackendRunner, DaemonControlEvent, OverlayActionIntents, OverlayState,
    VisibilityIntents,
};

// Some desktop custom shortcut runners, observed on KDE, can launch the same
// plain `--daemon-toggle` command twice about 400-600ms apart from one key press.
// Suppress only duplicate plain toggles after a successful toggle completes, so
// typed requests still run.
const DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW: Duration = Duration::from_millis(700);
// This bounds retries after journal I/O admission failures. It is unrelated to
// the removed tray startup-discovery fallback; retries use the existing v2 timerfd.
const ACTION_ADMISSION_RETRY_DELAY: Duration = Duration::from_millis(50);
#[cfg(unix)]
const DAEMON_SIGNALS: [libc::c_int; 3] = [libc::SIGUSR1, libc::SIGTERM, libc::SIGINT];
mod toggles;

fn finish_action_batch(failures: Vec<String>) -> Result<()> {
    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(failures.join("; ")))
    }
}

pub struct Daemon {
    pub(super) overlay_state: OverlayState,
    pub(super) should_quit: Arc<AtomicBool>,
    pub(super) visibility_intents: Arc<VisibilityIntents>,
    pub(super) initial_mode: Option<String>,
    pub(super) initial_named_session_file: Option<PathBuf>,
    pub(super) active_named_session_file: Option<PathBuf>,
    pub(super) instance_token: String,
    pub(super) freeze_on_show: bool,
    pub(super) tray_enabled: bool,
    pub(super) backend_runner: Option<Arc<BackendRunner>>,
    pub(super) tray_thread: Option<JoinHandle<()>>,
    pub(super) global_shortcuts_listener: Option<GlobalShortcutsListener>,
    pub(super) overlay_child: OverlayChildOwner,
    pub(super) overlay_active: Arc<AtomicBool>,
    pub(super) overlay_action_intents: Arc<OverlayActionIntents>,
    pub(super) pending_activation_token: Option<String>,
    pub(super) pending_toggle_request: Option<DaemonToggleRequest>,
    pub(super) session_resume_override: Arc<AtomicU8>,
    pub(super) lock_file: Option<std::fs::File>,
    pub(super) overlay_spawn_failures: u32,
    pub(super) overlay_spawn_next_retry: Option<std::time::Instant>,
    pub(super) overlay_spawn_backoff_logged: bool,
    pub(super) last_plain_visibility_toggle_completed_at: Option<Instant>,
    protocol_mode: DaemonControlProtocolMode,
    v2_command_owner: Option<CommandOwner>,
    v2_command_watcher: Option<CommandQueueWatcher>,
    v2_deadline_source: Option<BootDeadlineSource>,
    v2_action_journal: Option<ActionJournal>,
    pending_action_admission_retry: Vec<TrayAction>,
    action_admission_retry_at: Option<BootDeadline>,
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
            visibility_intents: Arc::new(VisibilityIntents::default()),
            initial_mode,
            initial_named_session_file,
            active_named_session_file: None,
            instance_token: crate::daemon::generate_daemon_instance_token(),
            freeze_on_show: false,
            tray_enabled,
            backend_runner: None,
            tray_thread: None,
            global_shortcuts_listener: None,
            overlay_child: OverlayChildOwner::default(),
            overlay_active: Arc::new(AtomicBool::new(false)),
            overlay_action_intents: Arc::new(OverlayActionIntents::default()),
            pending_activation_token: None,
            pending_toggle_request: None,
            session_resume_override: override_state,
            lock_file: None,
            overlay_spawn_failures: 0,
            overlay_spawn_next_retry: None,
            overlay_spawn_backoff_logged: false,
            last_plain_visibility_toggle_completed_at: None,
            protocol_mode: DaemonControlProtocolMode::production(),
            v2_command_owner: None,
            v2_command_watcher: None,
            v2_deadline_source: None,
            v2_action_journal: None,
            pending_action_admission_retry: Vec::new(),
            action_admission_retry_at: None,
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
            visibility_intents: Arc::new(VisibilityIntents::default()),
            initial_mode,
            initial_named_session_file: None,
            active_named_session_file: None,
            instance_token: crate::daemon::generate_daemon_instance_token(),
            freeze_on_show: false,
            tray_enabled: true,
            backend_runner: Some(backend_runner),
            tray_thread: None,
            global_shortcuts_listener: None,
            overlay_child: OverlayChildOwner::default(),
            overlay_active: Arc::new(AtomicBool::new(false)),
            overlay_action_intents: Arc::new(OverlayActionIntents::default()),
            pending_activation_token: None,
            pending_toggle_request: None,
            session_resume_override: override_state,
            lock_file: None,
            overlay_spawn_failures: 0,
            overlay_spawn_next_retry: None,
            overlay_spawn_backoff_logged: false,
            last_plain_visibility_toggle_completed_at: None,
            protocol_mode: DaemonControlProtocolMode::production(),
            v2_command_owner: None,
            v2_command_watcher: None,
            v2_deadline_source: None,
            v2_action_journal: None,
            pending_action_admission_retry: Vec::new(),
            action_admission_retry_at: None,
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

        let daemon_wake =
            RuntimeWakeSource::new().context("Failed to create daemon control wake descriptor")?;
        let visibility = self.visibility_intents.publisher(daemon_wake.handle());
        let action = self.overlay_action_intents.publisher(daemon_wake.handle());
        let quit_event = DaemonControlEvent::new(self.should_quit.clone(), daemon_wake.handle());

        #[cfg(unix)]
        {
            let listener_wake = daemon_wake.handle();
            let signal_visibility = visibility.clone();
            let signal_quit = quit_event.clone();
            self.signal_listener = Some(
                crate::unix_signals::spawn_listener(
                    &DAEMON_SIGNALS,
                    move |sig| {
                        if signal_quit.is_raised() {
                            return;
                        }
                        match sig {
                            libc::SIGUSR1 => {
                                info!("Received SIGUSR1 - toggling overlay");
                                if let Err(error) = signal_visibility.publish(
                                    None,
                                    true,
                                    "SIGUSR1 visibility publication",
                                ) {
                                    warn!("Failed to wake daemon after SIGUSR1: {error}");
                                }
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
                                if let Err(error) = signal_quit.raise("daemon shutdown signal") {
                                    warn!("Failed to wake daemon after shutdown signal: {error}");
                                }
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
        let publish_result = match self.protocol_mode {
            DaemonControlProtocolMode::LegacyV1 => {
                super::protocol_v2::prepare_rollback_compatibility()
                    .context("v2 state is not safe for rollback compatibility")?;
                crate::daemon::write_daemon_pid_file(std::process::id(), &self.instance_token)
            }
            #[cfg(test)]
            DaemonControlProtocolMode::DarkV2Harness => {
                unreachable!("dark harness must install protocol objects directly")
            }
            DaemonControlProtocolMode::PublishedV2 => {
                let token = ProtocolToken::generate()
                    .context("failed to generate daemon v2 instance token")?;
                let owner = CommandOwner::open(&token.to_string())
                    .context("failed to open daemon v2 command owner")?;
                super::protocol_v2::recover_stale_child_records()
                    .context("failed to recover daemon v2 child proofs")?;
                let watcher = CommandQueueWatcher::new(&owner.queue_path())
                    .context("failed to watch daemon v2 command queue")?;
                let deadline_source = BootDeadlineSource::new()
                    .context("failed to create daemon v2 deadline source")?;
                let action_journal =
                    ActionJournal::open().context("failed to open daemon v2 action journal")?;
                let runtime = DaemonRuntimeRecordV2::current(token)
                    .context("failed to build daemon v2 runtime identity")?;
                self.instance_token = runtime.v2_instance_token.clone();
                self.v2_command_owner = Some(owner);
                self.v2_command_watcher = Some(watcher);
                self.v2_deadline_source = Some(deadline_source);
                self.v2_action_journal = Some(action_journal);
                super::protocol_v2::write_runtime_record_v2(
                    &crate::paths::daemon_pid_file(),
                    &runtime,
                )
            }
        };
        if let Err(err) = publish_result {
            if let Err(stop_err) = self.stop_signal_listener() {
                warn!(
                    "Failed to stop signal listener after readiness publication error: {stop_err}"
                );
            }
            return Err(err);
        }

        // Start system tray (optional)
        if self.tray_enabled {
            let tray_overlay_active = self.overlay_active.clone();
            #[cfg(feature = "tray")]
            let tray_status = self.tray_status.clone();
            #[cfg(not(feature = "tray"))]
            let tray_status = ();
            match start_system_tray(
                visibility.clone(),
                action,
                quit_event.clone(),
                tray_overlay_active,
                tray_status,
            ) {
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
                self.global_shortcuts_listener =
                    start_global_shortcuts_listener(visibility, self.should_quit.clone());
                if self.global_shortcuts_listener.is_some() {
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
        if self.protocol_mode != DaemonControlProtocolMode::LegacyV1 {
            self.process_v2_commands()?;
        }
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

            // Action readiness and its FIFO batch are claimed under one mutex.
            // Visibility and its metadata are claimed separately immediately
            // afterward. A non-empty action batch intentionally absorbs that
            // coalesced visibility snapshot for compatibility with the existing
            // tray behavior.
            let (action_intents, claimed_admission_retry) = self.claim_overlay_action_batch()?;
            let visibility = self.visibility_intents.claim();
            if !action_intents.is_empty() {
                let result = self.process_overlay_action_intents(action_intents);
                if let Err(error) = result {
                    warn!("Overlay action batch failed: {error:#}");
                }
                if claimed_admission_retry
                    && self.pending_action_admission_retry.is_empty()
                    && self.overlay_action_intents.is_ready()
                {
                    continue;
                }
            } else if let Some(visibility) = visibility {
                let result = if self.protocol_mode == DaemonControlProtocolMode::LegacyV1 {
                    self.process_pending_toggles(
                        visibility.activation_token,
                        visibility.signal_requested,
                    )
                } else {
                    // In v2, raw SIGUSR1 and process-local shortcut/tray wakes
                    // are visibility-only. Typed queue discovery is exclusively
                    // driven by the watched v2 queue.
                    self.process_single_toggle(None, visibility.activation_token, false)
                        .map(drop)
                };
                if let Err(error) = result {
                    warn!("Toggle overlay failed: {error}");
                }
            }

            self.arm_v2_lifecycle_deadline()?;
            let readiness = wait_for_daemon_lifecycle(
                daemon_wake,
                self.v2_command_watcher.as_ref(),
                self.v2_deadline_source.as_ref(),
                &self.overlay_child,
            )?;
            if readiness.deadline {
                self.v2_deadline_source
                    .as_ref()
                    .context("v2 deadline source disappeared")?
                    .drain()
                    .context("failed to drain daemon v2 deadline source")?;
                self.process_v2_commands()?;
            }
            if readiness.command_queue {
                loop {
                    let drain = self
                        .v2_command_watcher
                        .as_mut()
                        .context("v2 command watcher disappeared")?
                        .drain()
                        .context("daemon v2 command queue watcher failed")?;
                    if drain.scan_pending {
                        self.process_v2_commands()?;
                    }
                    if !drain.more_pending {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn arm_v2_lifecycle_deadline(&self) -> Result<()> {
        let Some(source) = self.v2_deadline_source.as_ref() else {
            return Ok(());
        };
        let mut next = self.action_admission_retry_at;
        if let Some(owner) = self.v2_command_owner.as_ref()
            && let Some(command_deadline) = owner.next_maintenance_deadline()?
        {
            next = Some(next.map_or(command_deadline, |current| current.min(command_deadline)));
        }
        match next {
            Some(deadline) => source
                .arm(deadline)
                .context("failed to arm daemon v2 lifecycle deadline"),
            None => source
                .disarm()
                .context("failed to disarm daemon v2 lifecycle deadline"),
        }
    }

    fn claim_overlay_action_batch(&mut self) -> Result<(Vec<TrayAction>, bool)> {
        if self.pending_action_admission_retry.is_empty() {
            return Ok((self.overlay_action_intents.claim_batch(), false));
        }
        if let Some(retry_at) = self.action_admission_retry_at
            && BootClock::now()? < retry_at
        {
            return Ok((Vec::new(), false));
        }
        self.action_admission_retry_at = None;
        Ok((
            std::mem::take(&mut self.pending_action_admission_retry),
            true,
        ))
    }

    fn process_overlay_action_intents(&mut self, actions: Vec<TrayAction>) -> Result<()> {
        let action_count = actions.len();
        let mut failures = Vec::new();
        if self.protocol_mode == DaemonControlProtocolMode::LegacyV1 {
            for action in actions {
                if let Err(error) = self.process_single_toggle(
                    Some(DaemonToggleRequest {
                        overlay_action: Some(action),
                        ..Default::default()
                    }),
                    None,
                    false,
                ) {
                    failures.push(format!("{}: {error:#}", action.as_str()));
                }
            }
            self.overlay_action_intents.finish_batch(action_count);
            return finish_action_batch(failures);
        }

        let Some(journal) = self.v2_action_journal.clone() else {
            self.retain_action_admission_retry(actions, 0, &mut failures);
            failures.push("v2 action journal is not installed".to_string());
            return finish_action_batch(failures);
        };
        let mut admitted = Vec::with_capacity(actions.len());
        let mut retry = Vec::new();
        let mut will_be_visible = self.overlay_state == OverlayState::Visible;
        let mut actions = actions.into_iter();
        while let Some(action) = actions.next() {
            if !will_be_visible && matches!(action, crate::tray_action::TrayAction::LightDrawOff) {
                continue;
            }
            match journal.publish_anonymous(&self.instance_token, action) {
                Ok(prepared) => {
                    admitted.push((action, prepared));
                    will_be_visible = true;
                }
                Err(error) => {
                    failures.push(format!(
                        "failed to admit anonymous action {}: {error:#}",
                        action.as_str()
                    ));
                    retry.push(action);
                    retry.extend(actions);
                    break;
                }
            }
        }

        // Every admitted entry receives an explicit delivery or abandonment
        // disposition. Admission is completed for the batch before side effects
        // begin, so an early runtime failure cannot silently lose the tail.
        for (action, prepared) in admitted {
            if self.overlay_state == OverlayState::Hidden
                && matches!(action, crate::tray_action::TrayAction::LightDrawOff)
            {
                let reason = "overlay remained hidden before LightDrawOff delivery";
                if let Err(error) = journal.abandon(&prepared, reason) {
                    failures.push(format!(
                        "failed to abandon anonymous action {}: {error:#}",
                        action.as_str()
                    ));
                }
                continue;
            }

            let delivery = if self.overlay_state == OverlayState::Hidden {
                self.show_overlay()
                    .and_then(|()| self.signal_overlay_action_ready(action))
            } else {
                self.signal_overlay_action_ready(action)
            };
            if let Err(error) = delivery {
                let reason = format!("overlay action delivery failed: {error:#}");
                if let Err(abandon_error) = journal.abandon(&prepared, &reason) {
                    failures.push(format!(
                        "failed to abandon anonymous action {} after delivery failure: {abandon_error:#}",
                        action.as_str()
                    ));
                }
                failures.push(format!("{}: {reason}", action.as_str()));
            }
        }
        let completed = action_count - retry.len();
        self.retain_action_admission_retry(retry, completed, &mut failures);
        finish_action_batch(failures)
    }

    fn retain_action_admission_retry(
        &mut self,
        retry: Vec<TrayAction>,
        completed: usize,
        failures: &mut Vec<String>,
    ) {
        self.overlay_action_intents.finish_batch(completed);
        if retry.is_empty() {
            return;
        }
        self.pending_action_admission_retry.extend(retry);
        match BootClock::now().and_then(|now| now.checked_add(ACTION_ADMISSION_RETRY_DELAY)) {
            Ok(deadline) => {
                self.action_admission_retry_at = Some(
                    self.action_admission_retry_at
                        .map_or(deadline, |current| current.min(deadline)),
                );
            }
            Err(error) => failures.push(format!(
                "failed to schedule anonymous action admission retry: {error}"
            )),
        }
    }

    fn process_v2_commands(&mut self) -> Result<()> {
        loop {
            let claimed = self
                .v2_command_owner
                .as_ref()
                .context("v2 command owner is not installed")?
                .claim_next()?;
            let Some(mut claimed) = claimed else {
                break;
            };
            let request = claimed.request();
            let mut legacy_request: DaemonToggleRequest = request.into();
            if let Err(error) = legacy_request.normalize_and_validate_session_file() {
                claimed.reject(&format!("{error:#}"))?;
                claimed.defer()?;
                continue;
            }
            if let Err(error) =
                self.ensure_visible_overlay_can_accept_request(Some(&legacy_request))
            {
                claimed.reject(&format!("{error:#}"))?;
                claimed.defer()?;
                continue;
            }

            if let Some(action) = legacy_request.overlay_action {
                if self.overlay_state == OverlayState::Hidden
                    && matches!(action, crate::tray_action::TrayAction::LightDrawOff)
                {
                    claimed.commit(EffectKind::NoOp)?;
                    claimed.defer()?;
                    continue;
                }
                if !claimed.is_open() {
                    claimed.defer()?;
                    continue;
                }
                let journal = self
                    .v2_action_journal
                    .as_ref()
                    .context("v2 action journal is not installed")?
                    .clone();
                let command_identity = claimed.identity().to_owned();
                let Some(prepared) = claimed.prepare_action(&journal)? else {
                    claimed.defer()?;
                    continue;
                };
                let was_hidden = self.overlay_state == OverlayState::Hidden;
                claimed.commit(if was_hidden {
                    EffectKind::StartAndDeliverAction
                } else {
                    EffectKind::DeliverReadyAction
                })?;
                claimed.defer()?;

                self.pending_toggle_request = Some(legacy_request);
                if was_hidden {
                    if let Err(error) = self.show_overlay() {
                        let reason = format!("committed overlay start failed: {error:#}");
                        journal.abandon_command(&command_identity, &prepared, &reason)?;
                        warn!("{reason}");
                    } else if let Err(error) = self.signal_overlay_action_ready(action) {
                        let reason = format!("committed overlay wake failed: {error:#}");
                        journal.abandon_command(&command_identity, &prepared, &reason)?;
                        return Err(error).context(reason);
                    }
                } else {
                    if let Err(error) = self.signal_overlay_action_ready(action) {
                        let reason = format!("committed overlay wake failed: {error:#}");
                        journal.abandon_command(&command_identity, &prepared, &reason)?;
                        return Err(error).context(reason);
                    }
                    self.pending_toggle_request = None;
                }
                continue;
            }

            if let Some(effect) = claimed.authorized_effect() {
                claimed.finalize(
                    if effect == EffectKind::NoOp {
                        FinalEffect::Completed
                    } else {
                        FinalEffect::Indeterminate
                    },
                    (effect != EffectKind::NoOp).then_some(
                        "daemon resumed an authorized effect without terminal application proof",
                    ),
                )?;
                continue;
            }

            let effect = if self.overlay_state == OverlayState::Visible {
                EffectKind::HideReady
            } else {
                EffectKind::StartAndShow
            };
            claimed.commit(effect)?;
            // Typed requests are individually authorized and must not inherit
            // the legacy desktop-shortcut duplicate suppression window.
            self.last_plain_visibility_toggle_completed_at = None;
            match self.process_single_toggle(Some(legacy_request), None, false) {
                Ok(_) => claimed.finalize(FinalEffect::Completed, None)?,
                Err(error) => {
                    claimed.finalize(FinalEffect::Indeterminate, Some(&format!("{error:#}")))?
                }
            }
        }
        if let Some(owner) = self.v2_command_owner.as_ref() {
            owner.collect_terminal()?;
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
        if let Some(listener) = self.global_shortcuts_listener.as_mut() {
            listener.request_shutdown();
        }
        if let Some(handle) = self.tray_thread.take() {
            match handle.join() {
                Ok(()) => info!("System tray thread joined"),
                Err(err) => warn!("System tray thread panicked: {:?}", err),
            }
        }
        if let Some(listener) = self.global_shortcuts_listener.take() {
            match listener.join() {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DaemonLifecycleReadiness {
    command_queue: bool,
    deadline: bool,
}

fn wait_for_daemon_lifecycle(
    daemon_wake: &RuntimeWakeSource,
    command_watcher: Option<&CommandQueueWatcher>,
    deadline_source: Option<&BootDeadlineSource>,
    overlay_child: &OverlayChildOwner,
) -> Result<DaemonLifecycleReadiness> {
    let mut pollfds = vec![libc::pollfd {
        fd: daemon_wake.poll_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];
    if let Some(watcher) = command_watcher {
        pollfds.push(libc::pollfd {
            fd: watcher.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
    }
    let command_index = command_watcher.map(|_| 1);
    let deadline_index = deadline_source.map(|source| {
        let index = pollfds.len();
        pollfds.push(libc::pollfd {
            fd: source.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
        index
    });
    let child_index = overlay_child.poll_fd().map(|fd| {
        let index = pollfds.len();
        pollfds.push(libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
        index
    });
    loop {
        // SAFETY: the descriptor remains owned by `daemon_wake` throughout poll.
        let ready = unsafe {
            libc::poll(
                pollfds.as_mut_ptr(),
                pollfds
                    .len()
                    .try_into()
                    .expect("poll descriptor count fits"),
                -1,
            )
        };
        if ready == 0 {
            return Ok(DaemonLifecycleReadiness {
                command_queue: false,
                deadline: false,
            });
        }
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == ErrorKind::Interrupted {
                continue;
            }
            return Err(err).context("daemon lifecycle poll failed");
        }
        for pollfd in &pollfds {
            let terminal = pollfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL);
            if terminal != 0 {
                return Err(anyhow::anyhow!(
                    "daemon lifecycle descriptor returned terminal readiness {:#x}",
                    pollfd.revents
                ));
            }
        }
        let daemon_ready = pollfds[0].revents & libc::POLLIN != 0;
        let command_ready = command_index
            .and_then(|index| pollfds.get(index))
            .is_some_and(|pollfd| pollfd.revents & libc::POLLIN != 0);
        let deadline_ready = deadline_index
            .and_then(|index| pollfds.get(index))
            .is_some_and(|pollfd| pollfd.revents & libc::POLLIN != 0);
        let child_ready = child_index
            .and_then(|index| pollfds.get(index))
            .is_some_and(|pollfd| pollfd.revents & libc::POLLIN != 0);
        if !daemon_ready && !command_ready && !deadline_ready && !child_ready {
            return Err(anyhow::anyhow!(
                "daemon wake descriptor returned invalid readiness {:#x}",
                pollfds[0].revents
            ));
        }
        if daemon_ready {
            daemon_wake
                .drain()
                .context("failed to drain daemon wake descriptor")?;
        }
        return Ok(DaemonLifecycleReadiness {
            command_queue: command_ready,
            deadline: deadline_ready,
        });
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
