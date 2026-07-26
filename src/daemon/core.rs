use anyhow::{Context, Result};
use log::{info, warn};
use std::collections::VecDeque;
use std::fs;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::backend::wayland::RuntimeWakeSource;
use crate::env_vars::NO_TRAY_ENV;
use crate::session::try_lock_exclusive;
use crate::shortcut_hint::{ShortcutRuntimeBackend, current_shortcut_runtime_backend};
use crate::tray_action::{TrayAction, TrayActionQueue};

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
use super::tray::{TrayCleanupOwner, TrayRuntime, start_system_tray};
use super::types::{
    AlreadyRunningError, BackendRunner, DaemonControlMessage, DaemonEventInbox, OverlayState,
    VisibilityIntent, daemon_event_channel,
};
#[cfg(feature = "tray")]
use super::types::{AvailableUpdateNotice, TraySnapshot, TrayStatus};
use super::update_watch::{UpdateWatchHandle, start_update_watch};

// Some desktop custom shortcut runners, observed on KDE, can launch the same
// plain `--daemon-toggle` command twice about 400-600ms apart from one key press.
// Suppress only duplicate plain toggles after a successful toggle completes, so
// typed requests still run.
const DUPLICATE_SHORTCUT_SUPPRESSION_WINDOW: Duration = Duration::from_millis(700);
// This bounds retries after journal I/O admission failures. It is unrelated to
// the removed tray startup-discovery fallback; retries use the existing v2 timerfd.
const ACTION_ADMISSION_RETRY_DELAY: Duration = Duration::from_millis(50);
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
    pub(super) should_quit: bool,
    daemon_event_inbox: Option<DaemonEventInbox>,
    pending_visibility_intent: Option<VisibilityIntent>,
    pending_overlay_actions: VecDeque<TrayAction>,
    pub(super) tray_action_queue: TrayActionQueue,
    pub(super) initial_mode: Option<String>,
    pub(super) initial_named_session_file: Option<PathBuf>,
    pub(super) active_named_session_file: Option<PathBuf>,
    pub(super) instance_token: String,
    pub(super) freeze_on_show: bool,
    pub(super) tray_enabled: bool,
    pub(super) process_broker: Option<crate::process_broker::ProcessBrokerHandle>,
    pub(super) path_resolver: crate::paths::PathResolver,
    pub(super) runtime_paths: crate::paths::PreparedRuntimePaths,
    pub(super) config_store: crate::config::ConfigStore,
    pub(super) logger: crate::logger::LoggerHandle,
    pub(super) backend_runner: Option<Box<BackendRunner>>,
    pub(super) tray_runtime: Option<TrayRuntime>,
    tray_startup_cleanup: Option<TrayCleanupOwner>,
    pub(super) update_watch: Option<UpdateWatchHandle>,
    pub(super) global_shortcuts_listener: Option<GlobalShortcutsListener>,
    pub(super) overlay_child: OverlayChildOwner,
    pub(super) pending_activation_token: Option<String>,
    pub(super) pending_toggle_request: Option<DaemonToggleRequest>,
    pub(super) session_resume_override: Option<bool>,
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
    #[cfg(test)]
    _test_root: Option<crate::test_temp::TempDir>,
    #[cfg(feature = "tray")]
    pub(super) tray_status: TrayStatus,
}

pub(crate) struct DaemonLaunchOptions {
    pub(crate) initial_mode: Option<String>,
    pub(crate) tray_enabled: bool,
    pub(crate) session_resume_override: Option<bool>,
    pub(crate) initial_named_session_file: Option<PathBuf>,
}

pub(crate) struct DaemonRuntimeOwners {
    pub(crate) process_broker: crate::process_broker::ProcessBrokerHandle,
    pub(crate) path_resolver: crate::paths::PathResolver,
    pub(crate) runtime_paths: crate::paths::PreparedRuntimePaths,
    pub(crate) config_store: crate::config::ConfigStore,
    pub(crate) logger: crate::logger::LoggerHandle,
}

struct DaemonInternalOwners {
    process_broker: Option<crate::process_broker::ProcessBrokerHandle>,
    path_resolver: crate::paths::PathResolver,
    runtime_paths: crate::paths::PreparedRuntimePaths,
    config_store: crate::config::ConfigStore,
    logger: crate::logger::LoggerHandle,
}

impl Daemon {
    fn new_internal(
        options: DaemonLaunchOptions,
        owners: DaemonInternalOwners,
        backend_runner: Option<Box<BackendRunner>>,
    ) -> Self {
        let overlay_child = OverlayChildOwner::new(owners.runtime_paths.protocol_v2_root());
        Self {
            overlay_state: OverlayState::Hidden,
            should_quit: false,
            daemon_event_inbox: None,
            pending_visibility_intent: None,
            pending_overlay_actions: VecDeque::new(),
            tray_action_queue: TrayActionQueue::new(owners.runtime_paths.tray_action_dir()),
            initial_mode: options.initial_mode,
            initial_named_session_file: options.initial_named_session_file,
            active_named_session_file: None,
            instance_token: crate::daemon::generate_daemon_instance_token(),
            freeze_on_show: false,
            tray_enabled: options.tray_enabled,
            process_broker: owners.process_broker,
            path_resolver: owners.path_resolver,
            runtime_paths: owners.runtime_paths,
            config_store: owners.config_store,
            logger: owners.logger,
            backend_runner,
            tray_runtime: None,
            tray_startup_cleanup: None,
            update_watch: None,
            global_shortcuts_listener: None,
            overlay_child,
            pending_activation_token: None,
            pending_toggle_request: None,
            session_resume_override: options.session_resume_override,
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
            #[cfg(test)]
            _test_root: None,
            #[cfg(feature = "tray")]
            tray_status: TrayStatus::default(),
        }
    }

    #[cfg(test)]
    pub fn new(
        initial_mode: Option<String>,
        tray_enabled: bool,
        session_resume_override: Option<bool>,
        initial_named_session_file: Option<PathBuf>,
    ) -> Self {
        let test_root = crate::test_temp::tempdir()
            .expect("daemon test fixture creates its isolated path root");
        let path_resolver = daemon_test_path_resolver(&test_root);
        let runtime_paths = crate::paths::PreparedRuntimePaths::prepare(&path_resolver)
            .expect("daemon test fixture provides a private runtime directory");
        let config_store = crate::config::ConfigStore::from_resolver(&path_resolver)
            .expect("daemon test fixture provides a config identity");
        let mut daemon = Self::new_internal(
            DaemonLaunchOptions {
                initial_mode,
                tray_enabled,
                session_resume_override,
                initial_named_session_file,
            },
            DaemonInternalOwners {
                process_broker: None,
                path_resolver,
                runtime_paths,
                config_store,
                logger: crate::logger::LoggerHandle::discarding(),
            },
            None,
        );
        daemon._test_root = Some(test_root);
        daemon
    }

    #[cfg(test)]
    pub fn with_backend_runner(
        initial_mode: Option<String>,
        backend_runner: Box<BackendRunner>,
    ) -> Self {
        let test_root = crate::test_temp::tempdir()
            .expect("daemon test fixture creates its isolated path root");
        let path_resolver = daemon_test_path_resolver(&test_root);
        let runtime_paths = crate::paths::PreparedRuntimePaths::prepare(&path_resolver)
            .expect("daemon test fixture provides a private runtime directory");
        let config_store = crate::config::ConfigStore::from_resolver(&path_resolver)
            .expect("daemon test fixture provides a config identity");
        let mut daemon = Self::new_internal(
            DaemonLaunchOptions {
                initial_mode,
                tray_enabled: true,
                session_resume_override: None,
                initial_named_session_file: None,
            },
            DaemonInternalOwners {
                process_broker: None,
                path_resolver,
                runtime_paths,
                config_store,
                logger: crate::logger::LoggerHandle::discarding(),
            },
            Some(backend_runner),
        );
        daemon._test_root = Some(test_root);
        daemon
    }

    pub fn set_freeze_on_show(&mut self, enabled: bool) {
        self.freeze_on_show = enabled;
    }

    pub(crate) fn new_with_process_broker(
        options: DaemonLaunchOptions,
        owners: DaemonRuntimeOwners,
    ) -> Self {
        Self::new_internal(
            options,
            DaemonInternalOwners {
                process_broker: Some(owners.process_broker),
                path_resolver: owners.path_resolver,
                runtime_paths: owners.runtime_paths,
                config_store: owners.config_store,
                logger: owners.logger,
            },
            None,
        )
    }

    pub(super) fn effective_named_session_file(&self) -> Option<PathBuf> {
        self.pending_toggle_request
            .as_ref()
            .and_then(|request| request.session_file.clone())
            .or_else(|| self.initial_named_session_file.clone())
    }

    pub(super) fn session_resume_override(&self) -> Option<bool> {
        self.session_resume_override
    }

    fn acquire_daemon_lock(&mut self) -> Result<()> {
        let lock_path = self.runtime_paths.daemon_lock_file();
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

    /// Run the daemon with the root-owned external-event source.
    pub fn run(
        &mut self,
        signal_source: &mut dyn crate::unix_signals::SignalEventSource,
    ) -> Result<()> {
        self.logger
            .info("wayscriber::daemon", "daemon root started");
        let process_broker = self
            .process_broker
            .as_ref()
            .context("daemon requires an explicit process broker handle")?
            .clone();
        info!("Starting wayscriber daemon");
        if self.freeze_on_show {
            info!("Daemon activations will request frozen mode on show");
        }
        info!("Daemon control command: wayscriber --daemon-toggle [--freeze] [--mode …]");
        info!("Preferred external control: wayscriber --daemon-toggle");
        info!("Legacy raw SIGUSR1 toggle still works, but cannot carry launch args");

        self.acquire_daemon_lock()?;
        if let Err(err) = crate::daemon::clear_daemon_pid_file(&self.runtime_paths) {
            warn!("Failed to clear stale daemon pid file on startup: {}", err);
        }
        if let Err(err) = crate::daemon::clear_daemon_toggle_request_file(&self.runtime_paths) {
            warn!(
                "Failed to clear stale daemon toggle request on startup: {}",
                err
            );
        }

        let daemon_wake =
            RuntimeWakeSource::new().context("Failed to create daemon control wake descriptor")?;
        let (daemon_event_inbox, mut daemon_event_senders) = daemon_event_channel();
        self.daemon_event_inbox = Some(daemon_event_inbox);
        let visibility = daemon_event_senders.visibility(
            daemon_wake
                .try_sender()
                .context("Failed to duplicate daemon visibility wake descriptor")?,
        );
        let action = daemon_event_senders
            .overlay_actions(
                daemon_wake
                    .try_sender()
                    .context("Failed to duplicate daemon action wake descriptor")?,
            )
            .context("Failed to claim daemon overlay-action publisher")?;
        let quit_event = daemon_event_senders.quit(
            daemon_wake
                .try_sender()
                .context("Failed to duplicate daemon shutdown wake descriptor")?,
        );

        // The app root installed and blocked the daemon signals before any
        // ordinary runtime thread. Only publish the pid after this method owns
        // the descriptor consumer, so a racing legacy SIGUSR1 remains queued
        // for the control loop instead of taking its default action.
        let publish_result = match self.protocol_mode {
            DaemonControlProtocolMode::LegacyV1 => {
                super::protocol_v2::prepare_rollback_compatibility(&self.runtime_paths)
                    .context("v2 state is not safe for rollback compatibility")?;
                crate::daemon::write_daemon_pid_file(
                    std::process::id(),
                    &self.instance_token,
                    &self.runtime_paths,
                )
            }
            #[cfg(test)]
            DaemonControlProtocolMode::DarkV2Harness => Err(anyhow::anyhow!(
                "dark harness must install protocol objects directly"
            )),
            DaemonControlProtocolMode::PublishedV2 => {
                let token = ProtocolToken::generate()
                    .context("failed to generate daemon v2 instance token")?;
                let protocol_root = self.runtime_paths.protocol_v2_root();
                let owner = CommandOwner::open(&token.to_string(), protocol_root.clone())
                    .context("failed to open daemon v2 command owner")?;
                super::protocol_v2::recover_stale_child_records(&protocol_root)
                    .context("failed to recover daemon v2 child proofs")?;
                let watcher = CommandQueueWatcher::new(&owner.queue_path())
                    .context("failed to watch daemon v2 command queue")?;
                let deadline_source = BootDeadlineSource::new()
                    .context("failed to create daemon v2 deadline source")?;
                let action_journal = ActionJournal::open(protocol_root)
                    .context("failed to open daemon v2 action journal")?;
                let runtime = DaemonRuntimeRecordV2::current(token)
                    .context("failed to build daemon v2 runtime identity")?;
                self.instance_token = runtime.v2_instance_token.clone();
                self.v2_command_owner = Some(owner);
                self.v2_command_watcher = Some(watcher);
                self.v2_deadline_source = Some(deadline_source);
                self.v2_action_journal = Some(action_journal);
                super::protocol_v2::write_runtime_record_v2(
                    &self.runtime_paths.daemon_pid_file(),
                    &runtime,
                )
            }
        };
        publish_result?;

        // Start system tray (optional)
        if self.tray_enabled {
            #[cfg(feature = "tray")]
            let tray_status_publisher = daemon_event_senders.tray_status(
                daemon_wake
                    .try_sender()
                    .context("Failed to duplicate daemon tray-status wake descriptor")?,
            );
            #[cfg(not(feature = "tray"))]
            let tray_status_publisher = ();
            #[cfg(feature = "tray")]
            let tray_snapshot = self.tray_snapshot();
            #[cfg(not(feature = "tray"))]
            let tray_snapshot = ();
            match start_system_tray(
                super::tray::TrayControl {
                    visibility: visibility
                        .try_duplicate()
                        .context("Failed to duplicate daemon tray visibility wake descriptor")?,
                    action,
                    quit: quit_event
                        .try_duplicate()
                        .context("Failed to duplicate daemon tray shutdown wake descriptor")?,
                },
                tray_status_publisher,
                tray_snapshot,
                process_broker.clone(),
                self.config_store.clone(),
                self.path_resolver.clone(),
            ) {
                Ok(tray_runtime) => {
                    self.tray_runtime = Some(tray_runtime);
                }
                Err(failure) => {
                    let (err, cleanup) = failure.into_parts();
                    self.tray_startup_cleanup = cleanup;
                    warn!("System tray unavailable: {}", err);
                    warn!(
                        "Continuing without system tray; use --no-tray or {NO_TRAY_ENV}=1 to silence this warning"
                    );
                }
            }
        } else {
            info!("System tray disabled; running daemon without tray");
        }

        // Update notices are independent of the tray: without it the answer
        // still reaches the desktop notification and the About window.
        self.update_watch = start_update_watch(
            daemon_event_senders.update_watch(
                daemon_wake
                    .try_sender()
                    .context("Failed to duplicate update-watch wake descriptor")?,
            ),
            process_broker,
            self.config_store.clone(),
            crate::update_check::UpdateCacheStore::from_resolver(&self.path_resolver)?,
        );

        match current_shortcut_runtime_backend(&self.path_resolver) {
            ShortcutRuntimeBackend::PortalGlobalShortcuts => {
                self.global_shortcuts_listener = start_global_shortcuts_listener(visibility);
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

        let run_result =
            self.run_control_loop_and_invalidate_on_failure(&daemon_wake, signal_source);
        let cleanup_result = self.shutdown_after_run();
        run_result.and(cleanup_result)
    }

    fn run_control_loop_and_invalidate_on_failure(
        &mut self,
        daemon_wake: &RuntimeWakeSource,
        signal_source: &mut dyn crate::unix_signals::SignalEventSource,
    ) -> Result<()> {
        let result = self.run_control_loop(daemon_wake, signal_source);
        if result.is_err()
            && let Err(err) = crate::daemon::clear_daemon_pid_file(&self.runtime_paths)
        {
            warn!("Failed to invalidate daemon readiness after runtime failure: {err}");
        }
        result
    }

    fn run_control_loop(
        &mut self,
        daemon_wake: &RuntimeWakeSource,
        signal_source: &mut dyn crate::unix_signals::SignalEventSource,
    ) -> Result<()> {
        if self.protocol_mode != DaemonControlProtocolMode::LegacyV1 {
            self.process_v2_commands()?;
        }
        loop {
            self.drain_signal_events(signal_source)?;
            self.update_overlay_process_state()?;
            self.drain_daemon_events();
            self.sync_tray_snapshot();

            if self.should_quit {
                info!("Quit signal received - exiting daemon");
                break;
            }

            // The owner claims its FIFO action batch first, then its coalesced
            // visibility intent. A non-empty action batch intentionally absorbs
            // that visibility snapshot for compatibility with the existing tray
            // behavior.
            let (action_intents, claimed_admission_retry) = self.claim_overlay_action_batch()?;
            let visibility = self.pending_visibility_intent.take();
            if !action_intents.is_empty() {
                let result = self.process_overlay_action_intents(action_intents);
                if let Err(error) = result {
                    warn!("Overlay action batch failed: {error:#}");
                }
                if claimed_admission_retry
                    && self.pending_action_admission_retry.is_empty()
                    && !self.pending_overlay_actions.is_empty()
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
            self.sync_tray_snapshot();

            self.arm_v2_lifecycle_deadline()?;
            let readiness = wait_for_daemon_lifecycle(
                daemon_wake,
                signal_source,
                self.v2_command_watcher.as_ref(),
                self.v2_deadline_source.as_ref(),
                &self.overlay_child,
            )?;
            if readiness.signal {
                self.drain_signal_events(signal_source)?;
            }
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

    fn drain_daemon_events(&mut self) {
        let Some(inbox) = self.daemon_event_inbox.as_ref() else {
            return;
        };
        let batch = inbox.drain();
        self.pending_overlay_actions.extend(batch.overlay_actions);
        for message in batch.controls {
            match message {
                DaemonControlMessage::Quit => self.should_quit = true,
                DaemonControlMessage::Visibility(intent) => {
                    let pending = self
                        .pending_visibility_intent
                        .get_or_insert_with(VisibilityIntent::default);
                    if intent.activation_token.is_some() {
                        pending.activation_token = intent.activation_token;
                    }
                    pending.signal_requested |= intent.signal_requested;
                }
                DaemonControlMessage::UpdateAvailable(update) => {
                    #[cfg(feature = "tray")]
                    {
                        self.tray_status.available_update =
                            update.map(|update| AvailableUpdateNotice {
                                version: update.version,
                                update_url: update.update_url,
                            });
                    }
                    #[cfg(not(feature = "tray"))]
                    let _ = update;
                }
                DaemonControlMessage::UpdateNotificationAuthorization(request) => {
                    let authorized = self.overlay_state == OverlayState::Hidden;
                    match self.update_watch.as_ref() {
                        Some(watch) => {
                            if let Err(error) = watch.resolve_notification_authorization(
                                request.request_id,
                                request.update,
                                authorized,
                            ) {
                                log::debug!(
                                    "Update notification authorization could not reach watcher: {error}"
                                );
                            }
                        }
                        None => log::debug!(
                            "Update notification authorization arrived after watcher shutdown"
                        ),
                    }
                }
                #[cfg(feature = "tray")]
                DaemonControlMessage::TrayWatcherOnline => {
                    if self.tray_status.watcher_offline {
                        info!("StatusNotifierWatcher is online");
                    }
                    self.tray_status.watcher_offline = false;
                    self.tray_status.watcher_reason = None;
                }
                #[cfg(feature = "tray")]
                DaemonControlMessage::TrayWatcherOffline(reason) => {
                    if !self.tray_status.watcher_offline
                        || self.tray_status.watcher_reason.as_deref() != Some(reason.as_str())
                    {
                        warn!("StatusNotifierWatcher is offline: {reason}");
                    }
                    self.tray_status.watcher_offline = true;
                    self.tray_status.watcher_reason = Some(reason);
                }
            }
        }
    }

    fn drain_signal_events(
        &mut self,
        signal_source: &mut dyn crate::unix_signals::SignalEventSource,
    ) -> Result<()> {
        for event in signal_source
            .drain()
            .context("daemon signal source failed")?
        {
            match event {
                crate::unix_signals::SignalEvent::ToggleOverlay => {
                    info!("Received SIGUSR1 - toggling overlay");
                    self.pending_visibility_intent = Some(VisibilityIntent {
                        activation_token: None,
                        signal_requested: true,
                    });
                }
                crate::unix_signals::SignalEvent::Shutdown(signal) => {
                    let name = match signal {
                        crate::unix_signals::ShutdownSignal::Interrupt => "SIGINT",
                        crate::unix_signals::ShutdownSignal::Terminate => "SIGTERM",
                    };
                    info!("Received {name} - initiating graceful shutdown");
                    self.should_quit = true;
                }
                crate::unix_signals::SignalEvent::TrayAction => {
                    warn!("Daemon received an overlay-only tray signal event");
                }
            }
        }
        Ok(())
    }

    #[cfg(feature = "tray")]
    fn tray_snapshot(&self) -> TraySnapshot {
        TraySnapshot {
            overlay_active: self.overlay_state == OverlayState::Visible,
            status: self.tray_status.clone(),
        }
    }

    #[cfg(feature = "tray")]
    fn sync_tray_snapshot(&mut self) {
        let snapshot = self.tray_snapshot();
        if let Some(runtime) = self.tray_runtime.as_mut()
            && let Err(error) = runtime.sync(snapshot)
        {
            warn!("Failed to publish daemon state to system tray: {error}");
        }
    }

    #[cfg(not(feature = "tray"))]
    fn sync_tray_snapshot(&mut self) {}

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
            return Ok((self.pending_overlay_actions.drain(..).collect(), false));
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
            self.release_overlay_action_slots(action_count);
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
        self.release_overlay_action_slots(completed);
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

    fn release_overlay_action_slots(&self, completed: usize) {
        if completed == 0 {
            return;
        }
        let Some(inbox) = self.daemon_event_inbox.as_ref() else {
            return;
        };
        if let Err(error) = inbox.release_overlay_action_slots(completed) {
            log::debug!("Overlay-action publisher no longer accepts capacity releases: {error}");
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
        self.should_quit = true;
        self.stop_runtime_workers();
        if let Err(err) = crate::daemon::clear_daemon_toggle_request_file(&self.runtime_paths) {
            warn!("Failed to clear daemon toggle request file: {}", err);
        }
        if let Err(err) = crate::daemon::clear_daemon_pid_file(&self.runtime_paths) {
            warn!("Failed to clear daemon pid file: {}", err);
        }
        Ok(())
    }

    fn stop_runtime_workers(&mut self) {
        if let Some(listener) = self.global_shortcuts_listener.as_mut() {
            listener.request_shutdown();
        }
        if let Some(runtime) = self.tray_runtime.as_mut() {
            runtime.request_shutdown();
        }
        if let Some(cleanup) = self.tray_startup_cleanup.as_mut() {
            cleanup.request_shutdown();
        }
        if let Some(watch) = self.update_watch.as_mut() {
            watch.request_shutdown();
        }
        if let Some(runtime) = self.tray_runtime.take() {
            match runtime.join() {
                Ok(()) => info!("System tray thread joined"),
                Err(err) => warn!("System tray thread panicked: {:?}", err),
            }
        }
        if let Some(cleanup) = self.tray_startup_cleanup.take() {
            match cleanup.join() {
                Ok(()) => info!("Tray startup cleanup thread joined"),
                Err(err) => warn!("Tray startup cleanup thread panicked: {:?}", err),
            }
        }
        if let Some(watch) = self.update_watch.take() {
            match watch.join() {
                Ok(()) => info!("Update watcher thread joined"),
                Err(err) => warn!("Update watcher thread panicked: {:?}", err),
            }
        }
        if let Some(listener) = self.global_shortcuts_listener.take() {
            match listener.join() {
                Ok(()) => info!("Global shortcuts listener thread joined"),
                Err(err) => warn!("Global shortcuts listener thread panicked: {:?}", err),
            }
        }
    }
}

#[cfg(test)]
fn daemon_test_path_resolver(root: &crate::test_temp::TempDir) -> crate::paths::PathResolver {
    use crate::env_vars::{
        HOME_ENV, XDG_CACHE_HOME_ENV, XDG_CONFIG_HOME_ENV, XDG_DATA_HOME_ENV, XDG_RUNTIME_DIR_ENV,
    };

    let home = root.path().join("home");
    let config = root.path().join("config");
    let cache = root.path().join("cache");
    let data = root.path().join("data");
    let runtime = root.path().join("runtime");
    crate::paths::PathResolver::from_environment(crate::paths::PathEnvironment::for_test(&[
        (HOME_ENV, home.as_os_str()),
        (XDG_CONFIG_HOME_ENV, config.as_os_str()),
        (XDG_CACHE_HOME_ENV, cache.as_os_str()),
        (XDG_DATA_HOME_ENV, data.as_os_str()),
        (XDG_RUNTIME_DIR_ENV, runtime.as_os_str()),
    ]))
}

impl Drop for Daemon {
    fn drop(&mut self) {
        // `run` performs full daemon-state cleanup. This fallback owns only
        // worker lifetimes so construction errors and early drops cannot
        // detach a thread that inherited the root signal mask.
        self.stop_runtime_workers();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DaemonLifecycleReadiness {
    signal: bool,
    command_queue: bool,
    deadline: bool,
}

fn wait_for_daemon_lifecycle(
    daemon_wake: &RuntimeWakeSource,
    signal_source: &dyn crate::unix_signals::SignalEventSource,
    command_watcher: Option<&CommandQueueWatcher>,
    deadline_source: Option<&BootDeadlineSource>,
    overlay_child: &OverlayChildOwner,
) -> Result<DaemonLifecycleReadiness> {
    let mut pollfds = vec![libc::pollfd {
        fd: daemon_wake.poll_fd().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];
    let signal_index = pollfds.len();
    pollfds.push(libc::pollfd {
        fd: signal_source
            .poll_fd()
            .context("daemon signal descriptor is unavailable")?
            .as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    });
    if let Some(watcher) = command_watcher {
        pollfds.push(libc::pollfd {
            fd: watcher.poll_fd().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
    }
    let command_index = command_watcher.map(|_| 2);
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
        let poll_count: libc::nfds_t = pollfds
            .len()
            .try_into()
            .map_err(|_| anyhow::anyhow!("daemon lifecycle descriptor count exceeds nfds_t"))?;
        let ready = unsafe { libc::poll(pollfds.as_mut_ptr(), poll_count, -1) };
        if ready == 0 {
            return Ok(DaemonLifecycleReadiness {
                signal: false,
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
        let signal_ready = pollfds[signal_index].revents & libc::POLLIN != 0;
        let command_ready = command_index
            .and_then(|index| pollfds.get(index))
            .is_some_and(|pollfd| pollfd.revents & libc::POLLIN != 0);
        let deadline_ready = deadline_index
            .and_then(|index| pollfds.get(index))
            .is_some_and(|pollfd| pollfd.revents & libc::POLLIN != 0);
        let child_ready = child_index
            .and_then(|index| pollfds.get(index))
            .is_some_and(|pollfd| pollfd.revents & libc::POLLIN != 0);
        if !daemon_ready && !signal_ready && !command_ready && !deadline_ready && !child_ready {
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
            signal: signal_ready,
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
