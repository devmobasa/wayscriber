//! Background "a newer release exists" watcher.
//!
//! Wayscriber installs nothing. This thread only compares the running version
//! against the manifest published on wayscriber.com and surfaces the answer in
//! the tray and (once per release) as a desktop notification; the About window
//! reads the same cache. It never runs when the check is disabled by config or
//! by `WAYSCRIBER_DISABLE_UPDATE_CHECK`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use log::{debug, info, warn};

use crate::config::{Config, UpdatesConfig};
use crate::notification::NotificationError;
use crate::update_check::{self, AvailableUpdate, CachedStatus, CheckOutcome};

#[cfg(feature = "tray")]
use super::types::{AvailableUpdateNotice, TrayStatusShared};

/// How often the watcher wakes to ask whether a check is due. The gap between
/// actual requests comes from `[updates] interval_hours`.
const POLL_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Grace period before the first check, so a daemon started with the session is
/// not racing the network stack.
const STARTUP_DELAY: Duration = Duration::from_secs(45);

/// How soon a notification held back by an active overlay is retried.
const ANNOUNCE_RETRY: Duration = Duration::from_secs(60);

/// Sleep granularity, which bounds how long daemon shutdown waits for this
/// thread. Kept in the same ballpark as the tray's own poll loop.
const TICK: Duration = Duration::from_millis(250);

/// Notification icon: the freedesktop name desktops use for update prompts.
const NOTIFICATION_ICON: &str = "system-software-update";

/// Where a discovered update is published for the tray.
#[cfg(feature = "tray")]
type StatusSink = Arc<TrayStatusShared>;
#[cfg(not(feature = "tray"))]
type StatusSink = ();

/// Start the watcher, unless update checks are switched off.
pub(super) fn start_update_watch(
    quit: super::types::DaemonControlEvent,
    overlay_active: Arc<AtomicBool>,
    sink: StatusSink,
) -> Option<JoinHandle<()>> {
    let updates = load_updates_config();
    if !update_check::background_checks_enabled(updates.check) {
        info!("Update checks disabled; not starting the update watcher");
        return None;
    }

    info!(
        "Update watcher started (every {}h, notifications {})",
        updates.interval().as_secs() / 3600,
        if updates.notify { "on" } else { "off" }
    );

    Some(thread::spawn(move || {
        watch(quit, overlay_active, sink, updates);
    }))
}

fn watch(
    quit: super::types::DaemonControlEvent,
    overlay_active: Arc<AtomicBool>,
    sink: StatusSink,
    updates: UpdatesConfig,
) {
    // Seed from the cache first: a daemon restart should not lose a pending
    // notice, and reading the cache costs no network.
    let mut pending = update_from_cache();
    publish(&sink, pending.as_ref());

    let mut next_check = Instant::now() + STARTUP_DELAY;
    let mut next_announce = next_check;
    let mut announced = false;
    let mut check_throttle = update_check::CheckThrottle::default();

    while !quit.is_raised() {
        let now = Instant::now();

        if now >= next_check {
            if let Some(outcome) = check_throttle.check_if_due(updates.interval()) {
                pending = match outcome {
                    CheckOutcome::Update(update) => Some(update),
                    CheckOutcome::UpToDate { .. } => None,
                };
                announced = false;
                publish(&sink, pending.as_ref());
            }
            next_check = now + POLL_INTERVAL;
        }

        // Announcing is retried far more often than fetching, so a notice held
        // back by an active overlay lands shortly after the overlay closes
        // rather than at the next daily check.
        if !announced && now >= next_announce {
            announced = match pending.as_ref() {
                Some(update) => announce(update, updates.notify, &overlay_active),
                None => false,
            };
            next_announce = now + ANNOUNCE_RETRY;
        }

        thread::sleep(TICK);
    }

    debug!("Update watcher stopped");
}

/// Read `[updates]`, failing **closed**.
///
/// A missing config file is not an error (`Config::load` returns defaults), so
/// reaching the error arm means the file exists but could not be read or parsed
/// — exactly the case where a `check = false` the user wrote may be sitting
/// behind an unrelated syntax error. Defaulting to enabled there would make the
/// network request the user had switched off, so the check waits until the file
/// parses again.
fn load_updates_config() -> UpdatesConfig {
    match Config::load() {
        // A salvaged load with an unreadable [updates] section is the same
        // case as an unparseable file: a `check = false` the user wrote may
        // be sitting behind the bad value, so the policy still fails closed.
        Ok(loaded) if !loaded.section_failed("updates") => loaded.config.updates,
        Ok(_) => {
            warn!(
                "Skipping update checks until [updates] parses \
                 (cannot confirm the check/notify settings)"
            );
            UpdatesConfig {
                check: false,
                notify: false,
                ..UpdatesConfig::default()
            }
        }
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

fn update_from_cache() -> Option<AvailableUpdate> {
    match update_check::cached_status() {
        CachedStatus::Update { update, .. } => Some(update),
        CachedStatus::Never(_) | CachedStatus::UpToDate { .. } => None,
    }
}

/// Whether a desktop notification may be shown right now. An active annotation
/// session is never interrupted: the tray keeps the notice and, because nothing
/// is claimed, the next poll retries.
fn may_notify(notify_enabled: bool, overlay_active: bool) -> bool {
    notify_enabled && !overlay_active
}

/// Notify at most once per release. Returns whether this update is settled —
/// `false` means "try again later" (the overlay was up, or delivery failed).
fn announce(update: &AvailableUpdate, notify_enabled: bool, overlay_active: &AtomicBool) -> bool {
    if !notify_enabled {
        return true;
    }
    if !may_notify(notify_enabled, overlay_active.load(Ordering::Acquire)) {
        return false;
    }
    if !update_check::notification_pending(update) {
        return true;
    }
    match send_notification(update) {
        Ok(()) => {
            update_check::claim_notification(update);
            info!("Notified about Wayscriber {}", update.version);
            true
        }
        Err(NotificationError::Unavailable) => {
            debug!(
                "Update notification unavailable in this build; leaving Wayscriber {} pending",
                update.version
            );
            true
        }
        Err(NotificationError::Delivery(err)) => {
            debug!("Update notification not delivered: {err}");
            false
        }
    }
}

fn send_notification(update: &AvailableUpdate) -> Result<(), NotificationError> {
    let summary = format!("Wayscriber {} is available", update.version);
    let body = notification_body(update_check::current_version(), update);

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            warn!("Failed to create a runtime for the update notification: {err}");
            return Err(NotificationError::Delivery(format!(
                "Failed to create a runtime for the update notification: {err}"
            )));
        }
    };

    runtime.block_on(crate::notification::send_notification(
        &summary,
        &body,
        Some(NOTIFICATION_ICON),
    ))
}

/// The body states plainly that nothing was installed, and where the steps are.
fn notification_body(current: &str, update: &AvailableUpdate) -> String {
    format!(
        "You are running {current}. Wayscriber does not install updates itself — see {}",
        update.update_url
    )
}

#[cfg(feature = "tray")]
fn publish(sink: &StatusSink, update: Option<&AvailableUpdate>) {
    sink.set_available_update(update.map(|update| AvailableUpdateNotice {
        version: update.version.clone(),
        update_url: update.update_url.clone(),
    }));
}

#[cfg(not(feature = "tray"))]
fn publish(sink: &StatusSink, update: Option<&AvailableUpdate>) {
    let _ = (sink, update);
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
    fn an_active_overlay_or_a_disabled_setting_suppresses_the_notification() {
        assert!(may_notify(true, false));
        assert!(!may_notify(true, true));
        assert!(!may_notify(false, false));
        assert!(!may_notify(false, true));
    }

    #[test]
    fn an_active_overlay_defers_rather_than_settles_the_notice() {
        // Notifications off: settled, never retried.
        assert!(announce(&update(), false, &AtomicBool::new(false)));
        // Overlay up: unsettled, so the loop tries again shortly.
        assert!(!announce(&update(), true, &AtomicBool::new(true)));
    }

    #[cfg(not(feature = "dbus"))]
    #[test]
    fn unavailable_notifications_settle_without_claiming_the_release() {
        let _lock = crate::test_env::lock();
        let cache_home = crate::test_temp::tempdir().expect("temporary cache home");
        let previous = std::env::var_os(crate::env_vars::XDG_CACHE_HOME_ENV);
        unsafe {
            std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, cache_home.path());
        }

        let update = update();
        assert!(update_check::notification_pending(&update));
        assert!(announce(&update, true, &AtomicBool::new(false)));
        assert!(update_check::notification_pending(&update));

        match previous {
            Some(value) => unsafe {
                std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, value);
            },
            None => unsafe {
                std::env::remove_var(crate::env_vars::XDG_CACHE_HOME_ENV);
            },
        }
    }

    /// A broken config must not resurrect a check the user switched off.
    #[test]
    fn an_unparseable_config_disables_the_check_rather_than_defaulting_it_on() {
        crate::config::test_helpers::with_temp_config_home(|config_root| {
            let config_dir = config_root.join(crate::config::PRIMARY_CONFIG_DIR);
            std::fs::create_dir_all(&config_dir).unwrap();

            // A `check = false` the user wrote, plus an unrelated syntax error.
            std::fs::write(
                config_dir.join("config.toml"),
                "[updates]\ncheck = false\n\n[ui\nbroken = ",
            )
            .unwrap();
            let updates = load_updates_config();
            assert!(!updates.check);
            assert!(!updates.notify);
            assert!(!update_check::background_checks_enabled(updates.check));

            // The same file without the syntax error is honored as written.
            std::fs::write(config_dir.join("config.toml"), "[updates]\ncheck = false\n").unwrap();
            assert!(!load_updates_config().check);

            std::fs::write(config_dir.join("config.toml"), "[updates]\ncheck = true\n").unwrap();
            assert!(load_updates_config().check);
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
