//! Background "a newer release exists" watcher.
//!
//! Wayscriber installs nothing. This thread only compares the running version
//! against the manifest published on wayscriber.com and surfaces the answer in
//! the tray and (once per release) as a desktop notification; the About window
//! reads the same cache. It never runs when the check is disabled by config or
//! by `WAYSCRIBER_DISABLE_UPDATE_CHECK`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use log::{debug, info, warn};

use crate::config::UpdatesConfig;
use crate::update_check::{self, AvailableUpdate, CachedStatus, CheckOutcome};

use super::types::UpdateWatchPublisher;

/// How often the watcher wakes to ask whether a check is due. The gap between
/// actual requests comes from `[updates] interval_hours`.
const POLL_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Grace period before the first check, so a daemon started with the session is
/// not racing the network stack.
const STARTUP_DELAY: Duration = Duration::from_secs(45);

/// How soon a notification held back by an active overlay is retried.
const ANNOUNCE_RETRY: Duration = Duration::from_secs(60);

/// Idle command-wait granularity. An in-flight update fetch instead finishes
/// under its five-second helper deadline plus the process broker's bounded
/// exchange, queue, and response-delivery grace before this owner can join.
const TICK: Duration = Duration::from_millis(250);

/// Notification icon: the freedesktop name desktops use for update prompts.
const NOTIFICATION_ICON: &str = "system-software-update";

pub(super) struct UpdateWatchHandle {
    commands: Option<Sender<UpdateWatchCommand>>,
    thread: JoinHandle<()>,
}

#[derive(Debug)]
enum UpdateWatchCommand {
    ResolveNotification {
        request_id: u64,
        update: AvailableUpdate,
        authorized: bool,
    },
    Shutdown,
}

#[derive(Debug)]
pub(super) enum UpdateWatchCommandError {
    Closed,
    Disconnected,
}

impl std::fmt::Display for UpdateWatchCommandError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => formatter.write_str("update watcher command owner is closed"),
            Self::Disconnected => {
                formatter.write_str("update watcher command receiver disconnected")
            }
        }
    }
}

impl std::error::Error for UpdateWatchCommandError {}

impl UpdateWatchHandle {
    pub(super) fn request_shutdown(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(UpdateWatchCommand::Shutdown);
        }
    }

    pub(super) fn resolve_notification_authorization(
        &self,
        request_id: u64,
        update: AvailableUpdate,
        authorized: bool,
    ) -> Result<(), UpdateWatchCommandError> {
        let commands = self
            .commands
            .as_ref()
            .ok_or(UpdateWatchCommandError::Closed)?;
        commands
            .send(UpdateWatchCommand::ResolveNotification {
                request_id,
                update,
                authorized,
            })
            .map_err(|_| UpdateWatchCommandError::Disconnected)
    }

    pub(super) fn join(mut self) -> std::thread::Result<()> {
        self.request_shutdown();
        self.thread.join()
    }
}

/// Start the watcher, unless update checks are switched off.
pub(super) fn start_update_watch(
    publisher: UpdateWatchPublisher,
    process_broker: crate::process_broker::ProcessBrokerHandle,
    config_store: crate::config::ConfigStore,
    cache_store: update_check::UpdateCacheStore,
) -> Option<UpdateWatchHandle> {
    let updates = load_updates_config(&config_store);
    if !update_check::background_checks_enabled(updates.check) {
        info!("Update checks disabled; not starting the update watcher");
        return None;
    }

    info!(
        "Update watcher started (every {}h, notifications {})",
        updates.interval().as_secs() / 3600,
        if updates.notify { "on" } else { "off" }
    );

    let (command_sender, command_receiver) = mpsc::channel();
    let thread = thread::spawn(move || {
        watch(
            command_receiver,
            publisher,
            process_broker,
            updates,
            cache_store,
        );
    });
    Some(UpdateWatchHandle {
        commands: Some(command_sender),
        thread,
    })
}

fn watch(
    commands: Receiver<UpdateWatchCommand>,
    publisher: UpdateWatchPublisher,
    process_broker: crate::process_broker::ProcessBrokerHandle,
    updates: UpdatesConfig,
    cache_store: update_check::UpdateCacheStore,
) {
    // Seed from the cache first: a daemon restart should not lose a pending
    // notice, and reading the cache costs no network.
    let mut pending = update_from_cache(&cache_store);
    publish_available(&publisher, pending.clone());

    let mut next_check = Instant::now() + STARTUP_DELAY;
    let mut next_announce = next_check;
    let mut announced = false;
    let mut pending_authorization = None;
    let mut next_notification_request_id = 0_u64;
    let mut check_throttle = update_check::CheckThrottle::new(cache_store.clone());

    loop {
        let now = Instant::now();

        if now >= next_check {
            if let Some(outcome) = check_throttle.check_if_due(&process_broker, updates.interval())
            {
                pending = match outcome {
                    CheckOutcome::Update(update) => Some(update),
                    CheckOutcome::UpToDate { .. } => None,
                };
                announced = false;
                pending_authorization = None;
                publish_available(&publisher, pending.clone());
            }
            next_check = now + POLL_INTERVAL;
        }

        // Notification requests are retried far more often than fetching. The
        // daemon owner decides whether an overlay is active; a successful
        // delivery claims the cache record, which this worker observes on the
        // next retry without sharing state.
        if pending_authorization.is_some() && now >= next_announce {
            pending_authorization = None;
        }
        if !announced && pending_authorization.is_none() && now >= next_announce {
            match pending.as_ref() {
                Some(update) => match request_notification(
                    &publisher,
                    update,
                    updates.notify,
                    &cache_store,
                    &mut next_notification_request_id,
                ) {
                    NotificationRequestOutcome::Settled => announced = true,
                    NotificationRequestOutcome::Requested(request_id) => {
                        pending_authorization = Some(request_id);
                    }
                    NotificationRequestOutcome::Retry => {}
                },
                None => announced = false,
            }
            next_announce = now + ANNOUNCE_RETRY;
        }

        match receive_watch_commands(&commands, TICK) {
            WatchCommandBatch::Shutdown => break,
            WatchCommandBatch::Idle => {}
            WatchCommandBatch::NotificationDecisions(decisions) => {
                for decision in decisions {
                    if apply_notification_decision(
                        decision,
                        &mut pending_authorization,
                        pending.as_ref(),
                        &mut announced,
                        |update| deliver_notification(&cache_store, update),
                    ) {
                        next_announce = Instant::now() + ANNOUNCE_RETRY;
                    }
                }
            }
        }
    }

    debug!("Update watcher stopped");
}

#[derive(Debug)]
struct NotificationDecision {
    request_id: u64,
    update: AvailableUpdate,
    authorized: bool,
}

#[derive(Debug)]
enum WatchCommandBatch {
    Idle,
    NotificationDecisions(Vec<NotificationDecision>),
    Shutdown,
}

fn receive_watch_commands(
    commands: &Receiver<UpdateWatchCommand>,
    timeout: Duration,
) -> WatchCommandBatch {
    let first = match commands.recv_timeout(timeout) {
        Ok(command) => command,
        Err(mpsc::RecvTimeoutError::Timeout) => return WatchCommandBatch::Idle,
        Err(mpsc::RecvTimeoutError::Disconnected) => return WatchCommandBatch::Shutdown,
    };
    let mut received = vec![first];
    received.extend(commands.try_iter());
    if received
        .iter()
        .any(|command| matches!(command, UpdateWatchCommand::Shutdown))
    {
        return WatchCommandBatch::Shutdown;
    }
    WatchCommandBatch::NotificationDecisions(
        received
            .into_iter()
            .filter_map(|command| match command {
                UpdateWatchCommand::ResolveNotification {
                    request_id,
                    update,
                    authorized,
                } => Some(NotificationDecision {
                    request_id,
                    update,
                    authorized,
                }),
                UpdateWatchCommand::Shutdown => None,
            })
            .collect(),
    )
}

fn apply_notification_decision(
    decision: NotificationDecision,
    pending_request_id: &mut Option<u64>,
    pending_update: Option<&AvailableUpdate>,
    announced: &mut bool,
    deliver: impl FnOnce(&AvailableUpdate) -> bool,
) -> bool {
    if *pending_request_id != Some(decision.request_id) {
        return false;
    }
    *pending_request_id = None;
    let Some(update) = pending_update.filter(|update| *update == &decision.update) else {
        *announced = false;
        return true;
    };
    if decision.authorized {
        *announced = deliver(update);
    } else {
        *announced = false;
    }
    true
}

/// Read `[updates]`, failing **closed**.
///
/// A missing config file is not an error (`Config::load` returns defaults), so
/// reaching the error arm means the file exists but could not be read or parsed
/// — exactly the case where a `check = false` the user wrote may be sitting
/// behind an unrelated syntax error. Defaulting to enabled there would make the
/// network request the user had switched off, so the check waits until the file
/// parses again.
fn load_updates_config(config_store: &crate::config::ConfigStore) -> UpdatesConfig {
    match config_store.load() {
        Ok(loaded) => loaded.config.updates,
        Err(err) => {
            warn!(
                "Skipping update checks until the config file parses \
                 (cannot confirm the [updates] setting): {err}"
            );
            UpdatesConfig {
                check: false,
                notify: false,
                ..UpdatesConfig::default()
            }
        }
    }
}

fn update_from_cache(cache_store: &update_check::UpdateCacheStore) -> Option<AvailableUpdate> {
    match update_check::cached_status(cache_store) {
        CachedStatus::Update { update, .. } => Some(update),
        CachedStatus::Never(_) | CachedStatus::UpToDate { .. } => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NotificationRequestOutcome {
    Settled,
    Requested(u64),
    Retry,
}

fn request_notification(
    publisher: &UpdateWatchPublisher,
    update: &AvailableUpdate,
    notify_enabled: bool,
    cache_store: &update_check::UpdateCacheStore,
    next_request_id: &mut u64,
) -> NotificationRequestOutcome {
    if !notify_enabled {
        return NotificationRequestOutcome::Settled;
    }
    if !update_check::notification_pending(cache_store, update) {
        return NotificationRequestOutcome::Settled;
    }
    let request_id = *next_request_id;
    let Some(next) = request_id.checked_add(1) else {
        warn!("Update notification request identity space exhausted");
        return NotificationRequestOutcome::Retry;
    };
    *next_request_id = next;
    match publisher.request_notification(request_id, update.clone()) {
        Ok(()) => NotificationRequestOutcome::Requested(request_id),
        Err(error) => {
            debug!("Update notification request could not reach daemon owner: {error}");
            NotificationRequestOutcome::Retry
        }
    }
}

/// Deliver and claim at most one notification for this release. The daemon
/// owner has authorized this worker to notify while its overlay state is hidden.
fn deliver_notification(
    cache_store: &update_check::UpdateCacheStore,
    update: &AvailableUpdate,
) -> bool {
    if !update_check::notification_pending(cache_store, update) {
        return true;
    }
    if !send_notification(update) {
        return false;
    }
    update_check::claim_notification(cache_store, update);
    info!("Notified about Wayscriber {}", update.version);
    true
}

fn send_notification(update: &AvailableUpdate) -> bool {
    let summary = format!("Wayscriber {} is available", update.version);
    let body = notification_body(update_check::current_version(), update);

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            warn!("Failed to create a runtime for the update notification: {err}");
            return false;
        }
    };

    match runtime.block_on(crate::notification::send_notification(
        &summary,
        &body,
        Some(NOTIFICATION_ICON),
    )) {
        Ok(()) => true,
        Err(err) => {
            debug!("Update notification not delivered: {err}");
            false
        }
    }
}

/// The body states plainly that nothing was installed, and where the steps are.
fn notification_body(current: &str, update: &AvailableUpdate) -> String {
    format!(
        "You are running {current}. Wayscriber does not install updates itself — see {}",
        update.update_url
    )
}

fn publish_available(publisher: &UpdateWatchPublisher, update: Option<AvailableUpdate>) {
    if let Err(error) = publisher.publish_available(update) {
        debug!("Update availability could not reach daemon owner: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_check::{DEFAULT_NOTES_URL, DEFAULT_UPDATE_URL};

    fn update() -> AvailableUpdate {
        AvailableUpdate {
            version: "0.9.23".to_string(),
            released: Some("2026-07-24".to_string()),
            update_url: format!("{DEFAULT_UPDATE_URL}#ubuntu--debian"),
            notes_url: DEFAULT_NOTES_URL.to_string(),
        }
    }

    #[test]
    fn notification_body_says_nothing_was_installed() {
        let body = notification_body("0.9.22", &update());

        assert!(body.contains("0.9.22"));
        assert!(body.contains("does not install updates itself"));
        assert!(body.contains("#ubuntu--debian"));
    }

    #[test]
    fn a_disabled_notification_setting_settles_without_publication() {
        let (inbox, senders) = super::super::types::daemon_event_channel();
        let wake = crate::backend::wayland::RuntimeWakeSource::new()
            .expect("test creates a daemon runtime eventfd");
        let publisher = senders.update_watch(
            wake.try_sender()
                .expect("test duplicates its daemon runtime eventfd"),
        );
        let mut next_request_id = 0;
        let temp = crate::test_temp::tempdir().expect("isolated update cache fixture");
        let cache_store =
            update_check::UpdateCacheStore::at_path(temp.path().join("update-check.json"));

        assert_eq!(
            request_notification(
                &publisher,
                &update(),
                false,
                &cache_store,
                &mut next_request_id,
            ),
            NotificationRequestOutcome::Settled
        );
        assert_eq!(next_request_id, 0);
        assert!(inbox.drain().controls.is_empty());
    }

    #[test]
    fn notification_authorization_request_carries_identity_and_update() {
        let (inbox, senders) = super::super::types::daemon_event_channel();
        let wake = crate::backend::wayland::RuntimeWakeSource::new()
            .expect("test creates a daemon runtime eventfd");
        let publisher = senders.update_watch(
            wake.try_sender()
                .expect("test duplicates its daemon runtime eventfd"),
        );
        let expected_update = update();

        publisher
            .request_notification(41, expected_update.clone())
            .expect("test daemon event owner remains connected");

        let events = inbox.drain();
        assert!(matches!(
            events.controls.as_slice(),
            [super::super::types::DaemonControlMessage::UpdateNotificationAuthorization(
                request
            )] if request.request_id == 41 && request.update == expected_update
        ));
    }

    #[test]
    fn matching_authorization_is_delivered_and_claimed_by_the_watcher() {
        let expected_update = update();
        let mut pending_request_id = Some(7);
        let mut announced = false;
        let (delivered_sender, delivered_receiver) = mpsc::channel();

        let handled = apply_notification_decision(
            NotificationDecision {
                request_id: 7,
                update: expected_update.clone(),
                authorized: true,
            },
            &mut pending_request_id,
            Some(&expected_update),
            &mut announced,
            |delivered| delivered_sender.send(delivered.clone()).is_ok(),
        );

        assert!(handled);
        assert_eq!(pending_request_id, None);
        assert!(announced);
        assert_eq!(
            delivered_receiver
                .try_recv()
                .expect("matching authorization invokes the test delivery adapter"),
            expected_update
        );
    }

    #[test]
    fn denied_authorization_defers_without_delivery() {
        let expected_update = update();
        let mut pending_request_id = Some(9);
        let mut announced = false;
        let (delivered_sender, delivered_receiver) = mpsc::channel();

        let handled = apply_notification_decision(
            NotificationDecision {
                request_id: 9,
                update: expected_update.clone(),
                authorized: false,
            },
            &mut pending_request_id,
            Some(&expected_update),
            &mut announced,
            |delivered| delivered_sender.send(delivered.clone()).is_ok(),
        );

        assert!(handled);
        assert_eq!(pending_request_id, None);
        assert!(!announced);
        assert!(matches!(
            delivered_receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }

    #[test]
    fn queued_shutdown_preempts_queued_notification_delivery() {
        let (command_sender, command_receiver) = mpsc::channel();
        let (start_sender, start_receiver) = mpsc::channel();
        let (outcome_sender, outcome_receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            start_receiver
                .recv()
                .expect("test releases its watcher command receiver");
            let shutdown = matches!(
                receive_watch_commands(&command_receiver, Duration::from_secs(1)),
                WatchCommandBatch::Shutdown
            );
            outcome_sender
                .send(shutdown)
                .expect("test outcome receiver remains connected");
        });
        let mut handle = UpdateWatchHandle {
            commands: Some(command_sender),
            thread,
        };

        handle
            .resolve_notification_authorization(12, update(), true)
            .expect("test watcher command receiver remains connected");
        handle.request_shutdown();
        start_sender
            .send(())
            .expect("test watcher start receiver remains connected");
        handle
            .join()
            .expect("test watcher command thread exits normally");

        assert!(
            outcome_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("test watcher reports its shutdown outcome")
        );
    }

    /// A broken config must not resurrect a check the user switched off.
    #[test]
    fn an_unparseable_config_disables_the_check_rather_than_defaulting_it_on() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            let config_store = crate::config::test_helpers::test_config_store(config_root);
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            std::fs::create_dir_all(&config_dir)
                .expect("test creates its temporary config directory");

            // A `check = false` the user wrote, plus an unrelated syntax error.
            std::fs::write(
                config_dir.join("config.toml"),
                "[updates]\ncheck = false\n\n[ui\nbroken = ",
            )
            .expect("test writes an intentionally malformed config fixture");
            let updates = load_updates_config(&config_store);
            assert!(!updates.check);
            assert!(!updates.notify);
            assert!(!update_check::background_checks_enabled(updates.check));

            // The same file without the syntax error is honored as written.
            std::fs::write(config_dir.join("config.toml"), "[updates]\ncheck = false\n")
                .expect("test rewrites its config fixture with checks disabled");
            assert!(!load_updates_config(&config_store).check);

            std::fs::write(config_dir.join("config.toml"), "[updates]\ncheck = true\n")
                .expect("test rewrites its config fixture with checks enabled");
            assert!(load_updates_config(&config_store).check);
        });
    }

    #[test]
    fn the_watcher_polls_far_less_often_than_it_ticks() {
        assert!(TICK < ANNOUNCE_RETRY);
        assert!(ANNOUNCE_RETRY < STARTUP_DELAY + POLL_INTERVAL);
        assert!(STARTUP_DELAY < POLL_INTERVAL);
        // A daily check must not be defeated by the poll cadence.
        assert!(POLL_INTERVAL < UpdatesConfig::default().interval());
    }
}
