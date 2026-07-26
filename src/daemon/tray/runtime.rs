#[cfg(feature = "tray")]
use anyhow::Result;
#[cfg(feature = "tray")]
use anyhow::anyhow;
#[cfg(feature = "tray")]
use ksni::TrayMethods;
use log::info;
#[cfg(feature = "tray")]
use log::{debug, warn};
#[cfg(feature = "tray")]
use std::sync::mpsc;
#[cfg(feature = "tray")]
use std::thread;
use std::thread::JoinHandle;
#[cfg(feature = "tray")]
use std::time::Duration;
#[cfg(feature = "tray")]
use zbus::{Connection, Proxy};

#[cfg(feature = "tray")]
use super::super::types::{TraySnapshot, TrayStatusPublisher};
#[cfg(feature = "tray")]
use super::WayscriberTray;
#[cfg(feature = "tray")]
use crate::config::TrayIconStyle;
#[cfg(feature = "tray")]
use crate::env_vars::CONFIGURATOR_ENV;

#[cfg(feature = "tray")]
const TRAY_START_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "tray")]
const TRAY_START_WAIT_TIMEOUT: Duration = Duration::from_secs(6);
#[cfg(feature = "tray")]
const TRAY_OPERATION_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(feature = "tray")]
const TRAY_COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(100);
#[cfg(feature = "tray")]
const STATUS_NOTIFIER_WATCHER_BUS: &str = "org.kde.StatusNotifierWatcher";
#[cfg(feature = "tray")]
const STATUS_NOTIFIER_WATCHER_PATH: &str = "/StatusNotifierWatcher";

#[cfg(feature = "tray")]
enum TrayCommand {
    Sync(TraySnapshot),
    Shutdown,
}

#[cfg(feature = "tray")]
enum TrayStartupOutcome {
    Ready,
    Failed(anyhow::Error),
    TimedOut,
    Disconnected,
}

#[cfg(feature = "tray")]
pub(crate) struct TrayRuntime {
    commands: Option<mpsc::Sender<TrayCommand>>,
    thread: Option<JoinHandle<()>>,
    last_snapshot: TraySnapshot,
}

#[cfg(feature = "tray")]
impl TrayRuntime {
    pub(crate) fn sync(&mut self, snapshot: TraySnapshot) -> Result<()> {
        if snapshot == self.last_snapshot {
            return Ok(());
        }
        let commands = self
            .commands
            .as_ref()
            .ok_or_else(|| anyhow!("tray runtime command owner is closed"))?;
        commands
            .send(TrayCommand::Sync(snapshot.clone()))
            .map_err(|_| anyhow!("tray runtime command receiver disconnected"))?;
        self.last_snapshot = snapshot;
        Ok(())
    }

    pub(crate) fn request_shutdown(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(TrayCommand::Shutdown);
        }
    }

    fn finish(&mut self) -> std::thread::Result<()> {
        self.request_shutdown();
        match self.thread.take() {
            Some(thread) => thread.join(),
            None => Ok(()),
        }
    }

    pub(crate) fn join(mut self) -> std::thread::Result<()> {
        self.finish()
    }
}

#[cfg(feature = "tray")]
impl Drop for TrayRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            warn!("System tray thread panicked during owner drop: {error:?}");
        }
    }
}

#[cfg(not(feature = "tray"))]
pub(crate) struct TrayRuntime {
    thread: Option<JoinHandle<()>>,
}

#[cfg(not(feature = "tray"))]
impl TrayRuntime {
    pub(crate) fn request_shutdown(&mut self) {}

    fn finish(&mut self) -> std::thread::Result<()> {
        match self.thread.take() {
            Some(thread) => thread.join(),
            None => Ok(()),
        }
    }

    pub(crate) fn join(mut self) -> std::thread::Result<()> {
        self.finish()
    }
}

#[cfg(not(feature = "tray"))]
impl Drop for TrayRuntime {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

pub(crate) struct TrayCleanupOwner {
    #[cfg(feature = "tray")]
    commands: Option<mpsc::Sender<TrayCommand>>,
    thread: Option<JoinHandle<()>>,
}

impl TrayCleanupOwner {
    #[cfg(feature = "tray")]
    fn start(commands: mpsc::Sender<TrayCommand>, thread: JoinHandle<()>, reason: &str) -> Self {
        match commands.send(TrayCommand::Shutdown) {
            Ok(()) => debug!("Queued tray startup cleanup after {reason}"),
            Err(_) => debug!("Tray startup worker already exited after {reason}"),
        }
        Self {
            commands: Some(commands),
            thread: Some(thread),
        }
    }

    pub(crate) fn request_shutdown(&mut self) {
        #[cfg(feature = "tray")]
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(TrayCommand::Shutdown);
        }
    }

    fn finish(&mut self) -> std::thread::Result<()> {
        self.request_shutdown();
        match self.thread.take() {
            Some(thread) => thread.join(),
            None => Ok(()),
        }
    }

    pub(crate) fn join(mut self) -> std::thread::Result<()> {
        self.finish()
    }
}

impl Drop for TrayCleanupOwner {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            log::warn!("Tray startup cleanup thread panicked during owner drop: {error:?}");
        }
    }
}

pub(crate) struct TrayStartupFailure {
    error: anyhow::Error,
    cleanup: Option<TrayCleanupOwner>,
}

impl TrayStartupFailure {
    #[cfg(feature = "tray")]
    fn unstarted(error: anyhow::Error) -> Self {
        Self {
            error,
            cleanup: None,
        }
    }

    #[cfg(feature = "tray")]
    fn cleaning_up(error: anyhow::Error, cleanup: TrayCleanupOwner) -> Self {
        Self {
            error,
            cleanup: Some(cleanup),
        }
    }

    pub(crate) fn into_parts(self) -> (anyhow::Error, Option<TrayCleanupOwner>) {
        (self.error, self.cleanup)
    }
}

#[cfg(feature = "tray")]
fn load_tray_settings_from_config(
    config_store: &crate::config::ConfigStore,
) -> (bool, TrayIconStyle) {
    match config_store.load() {
        Ok(loaded) => {
            let config = loaded.config;
            let session = config.session;
            let session_resume_enabled = session.persist_transparent
                || session.persist_whiteboard
                || session.persist_blackboard
                || session.persist_history
                || session.restore_tool_state;
            (session_resume_enabled, config.tray.icon_style)
        }
        Err(err) => {
            warn!(
                "Failed to read config for tray settings; using safe defaults: {}",
                err
            );
            (false, TrayIconStyle::Auto)
        }
    }
}

#[cfg(feature = "tray")]
pub(super) fn update_session_resume_in_config(
    config_store: &crate::config::ConfigStore,
    target_enabled: bool,
    fallback: bool,
) -> bool {
    match config_store.load() {
        Ok(loaded) => {
            let mut config = loaded.config;
            config.session.persist_transparent = target_enabled;
            config.session.persist_whiteboard = target_enabled;
            config.session.persist_blackboard = target_enabled;
            config.session.persist_history = target_enabled;
            config.session.restore_tool_state = target_enabled;
            if let Err(err) = config_store.save(&config) {
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

/// System tray implementation
#[cfg(feature = "tray")]
pub(crate) fn start_system_tray(
    control: super::TrayControl,
    status_publisher: TrayStatusPublisher,
    initial_snapshot: TraySnapshot,
    process_broker: crate::process_broker::ProcessBrokerHandle,
    config_store: crate::config::ConfigStore,
    path_resolver: crate::paths::PathResolver,
) -> std::result::Result<TrayRuntime, TrayStartupFailure> {
    let configurator_binary =
        std::env::var(CONFIGURATOR_ENV).unwrap_or_else(|_| "wayscriber-configurator".to_string());
    let (session_resume_enabled, icon_style) = load_tray_settings_from_config(&config_store);

    let tray = WayscriberTray::new(
        control,
        super::TraySetup {
            configurator_binary,
            session_resume_enabled,
            icon_style,
            snapshot: initial_snapshot.clone(),
            status_publisher,
            process_broker,
            config_store,
            path_resolver,
        },
    );
    let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
    let (command_tx, command_rx) = mpsc::channel();

    info!("Creating tray service...");
    info!("Spawning system tray runtime thread...");

    let tray_thread = thread::Builder::new()
        .name("wayscriber-system-tray".into())
        .spawn(move || {
            // A current-thread runtime keeps every tray task inside this
            // root-owned worker. Once the bounded outer operation finishes,
            // dropping the runtime cannot leave Tokio worker threads behind.
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => {
                    warn!("Failed to create Tokio runtime for system tray: {}", e);
                    report_tray_readiness(
                        &ready_tx,
                        Err(anyhow!(
                            "Failed to create Tokio runtime for system tray: {e}"
                        )),
                    );
                    return;
                }
            };

            rt.block_on(async {
                match tokio::time::timeout(
                    TRAY_START_TIMEOUT,
                    tray.assume_sni_available(true).spawn(),
                )
                .await
                {
                    Ok(Ok(handle)) => {
                        info!("System tray spawned successfully");
                        report_tray_readiness(&ready_tx, Ok(()));
                        tokio::spawn(log_status_notifier_state());

                        loop {
                            tokio::time::sleep(TRAY_COMMAND_POLL_INTERVAL).await;
                            let mut latest_snapshot = None;
                            let mut shutdown = false;
                            loop {
                                match command_rx.try_recv() {
                                    Ok(TrayCommand::Sync(snapshot)) => {
                                        latest_snapshot = Some(snapshot);
                                    }
                                    Ok(TrayCommand::Shutdown) => {
                                        shutdown = true;
                                        break;
                                    }
                                    Err(mpsc::TryRecvError::Empty) => break,
                                    Err(mpsc::TryRecvError::Disconnected) => {
                                        shutdown = true;
                                        break;
                                    }
                                }
                            }
                            if shutdown {
                                info!("Shutdown requested - stopping system tray");
                                if tokio::time::timeout(TRAY_OPERATION_TIMEOUT, handle.shutdown())
                                    .await
                                    .is_err()
                                {
                                    warn!("Timed out while shutting down the system tray");
                                }
                                break;
                            }
                            if let Some(snapshot) = latest_snapshot {
                                match tokio::time::timeout(
                                    TRAY_OPERATION_TIMEOUT,
                                    handle.update(move |tray| tray.apply_snapshot(snapshot)),
                                )
                                .await
                                {
                                    Ok(Some(_)) => {}
                                    Ok(None) => {
                                        warn!("Tray service closed; stopping tray monitor");
                                        break;
                                    }
                                    Err(_) => {
                                        warn!("Timed out while updating the system tray");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        warn!("System tray error: {}", e);
                        report_tray_readiness(&ready_tx, Err(anyhow!("System tray error: {e}")));
                    }
                    Err(_) => {
                        warn!("Timed out while spawning the system tray");
                        report_tray_readiness(
                            &ready_tx,
                            Err(anyhow!("Timed out while spawning the system tray")),
                        );
                    }
                }
            });
        })
        .map_err(|error| {
            TrayStartupFailure::unstarted(anyhow!("Failed to start system tray worker: {error}"))
        })?;

    info!("Waiting for system tray readiness signal...");
    match wait_for_tray_readiness(&ready_rx, TRAY_START_WAIT_TIMEOUT) {
        TrayStartupOutcome::Ready => {
            info!("System tray thread started");
            Ok(TrayRuntime {
                commands: Some(command_tx),
                thread: Some(tray_thread),
                last_snapshot: initial_snapshot,
            })
        }
        TrayStartupOutcome::Failed(error) => {
            let cleanup = TrayCleanupOwner::start(command_tx, tray_thread, "tray startup failed");
            Err(TrayStartupFailure::cleaning_up(error, cleanup))
        }
        TrayStartupOutcome::TimedOut => {
            warn!("Timed out waiting for system tray to start");
            let cleanup =
                TrayCleanupOwner::start(command_tx, tray_thread, "tray readiness timed out");
            Err(TrayStartupFailure::cleaning_up(
                anyhow!("Timed out waiting for system tray to start"),
                cleanup,
            ))
        }
        TrayStartupOutcome::Disconnected => {
            let cleanup = TrayCleanupOwner::start(
                command_tx,
                tray_thread,
                "tray readiness channel disconnected",
            );
            Err(TrayStartupFailure::cleaning_up(
                anyhow!("System tray thread exited before signaling readiness"),
                cleanup,
            ))
        }
    }
}

#[cfg(feature = "tray")]
fn wait_for_tray_readiness(
    ready: &mpsc::Receiver<Result<()>>,
    timeout: Duration,
) -> TrayStartupOutcome {
    match ready.recv_timeout(timeout) {
        Ok(Ok(())) => TrayStartupOutcome::Ready,
        Ok(Err(error)) => TrayStartupOutcome::Failed(error),
        Err(mpsc::RecvTimeoutError::Timeout) => TrayStartupOutcome::TimedOut,
        Err(mpsc::RecvTimeoutError::Disconnected) => TrayStartupOutcome::Disconnected,
    }
}

#[cfg(not(feature = "tray"))]
pub(crate) fn start_system_tray(
    control: super::TrayControl,
    _status_publisher: (),
    _initial_snapshot: (),
    _process_broker: crate::process_broker::ProcessBrokerHandle,
    _config_store: crate::config::ConfigStore,
    _path_resolver: crate::paths::PathResolver,
) -> std::result::Result<TrayRuntime, TrayStartupFailure> {
    let super::TrayControl {
        visibility,
        action,
        quit,
    } = control;
    drop((visibility, action, quit));
    info!("Tray feature disabled; skipping system tray startup");
    Ok(TrayRuntime { thread: None })
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
            warn!(
                "Failed to connect to session D-Bus for tray diagnostics: {}",
                err
            );
            return;
        }
    };

    let proxy = match Proxy::new(
        &conn,
        STATUS_NOTIFIER_WATCHER_BUS,
        STATUS_NOTIFIER_WATCHER_PATH,
        STATUS_NOTIFIER_WATCHER_BUS,
    )
    .await
    {
        Ok(proxy) => proxy,
        Err(err) => {
            warn!("StatusNotifierWatcher unavailable (no tray host?): {}", err);
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
    use std::time::Instant;

    #[test]
    fn readiness_wait_is_bounded_when_the_worker_stays_silent() {
        let (_ready_sender, ready_receiver) = mpsc::channel::<Result<()>>();
        let started = Instant::now();

        let outcome = wait_for_tray_readiness(&ready_receiver, Duration::from_millis(20));

        assert!(matches!(outcome, TrayStartupOutcome::TimedOut));
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn failed_startup_cleanup_is_owned_and_joinable() {
        let (command_sender, command_receiver) = mpsc::channel();
        let (outcome_sender, outcome_receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let shutdown = matches!(
                command_receiver.recv_timeout(Duration::from_secs(1)),
                Ok(TrayCommand::Shutdown)
            );
            outcome_sender
                .send(shutdown)
                .expect("test cleanup outcome receiver remains connected");
        });

        let cleanup = TrayCleanupOwner::start(command_sender, thread, "test timeout");

        assert!(
            outcome_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("test tray worker reports the cleanup command")
        );
        cleanup
            .join()
            .expect("test joins its tray startup cleanup owner");
    }
}
