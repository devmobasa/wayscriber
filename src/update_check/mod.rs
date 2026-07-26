//! Opt-outable "a newer version exists" check.
//!
//! Wayscriber never updates itself. The check downloads one static JSON file
//! from wayscriber.com, compares its version to this build, and — at most once
//! per release — points the user at the update instructions for their install
//! method. Everything else (downloading, installing, restarting) stays with the
//! package manager, where it belongs.
//!
//! Layout:
//! - [`version`]: semver-ish ordering, so only a genuinely newer release counts
//! - [`manifest`]: the published document plus URL trust rules
//! - [`fetch`]: system `curl`/`wget` transport
//! - [`cache`]: last result, throttle timestamp, notification dedupe

mod cache;
mod fetch;
mod manifest;
mod version;

use std::time::Duration;

use log::debug;

pub(crate) use cache::UpdateCacheStore;
pub use manifest::{DEFAULT_NOTES_URL, DEFAULT_UPDATE_URL, MANIFEST_URL, install_source};

use crate::env_vars::DISABLE_UPDATE_CHECK_ENV;

/// Network timeout for one manifest request.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Shown wherever a check is requested from a build that has it compiled out.
pub const COMPILED_OUT_MESSAGE: &str = "This build has the update check disabled";

/// A release newer than the running build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableUpdate {
    /// Version string as published (no `v` prefix).
    pub version: String,
    /// Publication date, when the manifest carries one.
    pub released: Option<String>,
    /// Install-method-specific update instructions.
    pub update_url: String,
    /// Release notes for the new version.
    pub notes_url: String,
}

/// Result of a check that actually ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    UpToDate { latest: String },
    Update(AvailableUpdate),
}

/// How current a cached answer is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Freshness {
    /// Age of the last check that actually got an answer.
    pub checked_seconds_ago: Option<u64>,
    /// The most recent attempt failed, so the stored answer is unconfirmed.
    /// Reported separately from the age because "checked 3 hours ago" would
    /// otherwise hide a check that failed thirty seconds ago.
    pub last_attempt_failed: bool,
}

/// What the About window shows without touching the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CachedStatus {
    /// No check has ever completed successfully on this machine. Freshness
    /// distinguishes a first failed attempt from no attempt at all.
    Never(Freshness),
    UpToDate(Freshness),
    Update {
        update: AvailableUpdate,
        freshness: Freshness,
    },
}

/// The running build's version.
pub fn current_version() -> &'static str {
    crate::build_info::version()
}

/// Whether this build can check at all.
///
/// Packagers who must guarantee no outbound requests build with
/// `WAYSCRIBER_NO_UPDATE_CHECK=1`; nothing at runtime can re-enable it.
pub const fn compiled_out() -> bool {
    option_env!("WAYSCRIBER_NO_UPDATE_CHECK").is_some()
}

/// Whether periodic background checking is allowed.
///
/// `WAYSCRIBER_DISABLE_UPDATE_CHECK` wins over the config file so a user (or a
/// distro's session wrapper) can kill the check without editing config. An
/// explicit `--check-update` run ignores both: asking for a check *is* consent.
pub fn background_checks_enabled(config_enabled: bool) -> bool {
    if compiled_out() || env_opt_out() {
        return false;
    }
    config_enabled
}

/// Read the opt-out variable.
///
/// The documented falsey words (`0`, `false`, `no`, `off`, `disable`,
/// `disabled`, and empty) mean "do not opt out", matching the rest of
/// Wayscriber's environment parsing. Anything else counts as an opt-out: for a
/// switch whose only job is to stop network access, an unrecognized value is
/// honored rather than ignored, so a typo cannot silently re-enable the check.
fn env_opt_out() -> bool {
    env_value_opts_out(std::env::var_os(DISABLE_UPDATE_CHECK_ENV).as_deref())
}

fn env_value_opts_out(value: Option<&std::ffi::OsStr>) -> bool {
    value
        .map(|value| {
            let value = value.to_string_lossy().trim().to_ascii_lowercase();
            !matches!(
                value.as_str(),
                "" | "0" | "false" | "no" | "off" | "disable" | "disabled"
            )
        })
        .unwrap_or(false)
}

/// Report the last stored result without any network access.
pub(crate) fn cached_status(cache_store: &UpdateCacheStore) -> CachedStatus {
    let cache = cache_store.load();
    // Age is measured from the last *successful* fetch, and a newer failed
    // attempt is reported alongside it, so a stale answer can neither look
    // freshly verified nor hide the failed retry.
    let freshness = Freshness {
        checked_seconds_ago: cache::seconds_since(cache.last_success_unix),
        // Older schemas could not represent every sequence reliably, so their
        // migrated outcome stays `None` instead of guessing from timestamps.
        last_attempt_failed: matches!(
            cache.last_attempt_outcome,
            Some(cache::AttemptOutcome::Failed)
        ),
    };
    let Some(latest) = cache.latest_version.as_deref() else {
        return CachedStatus::Never(freshness);
    };

    match update_from_cache(&cache, latest) {
        Some(update) => CachedStatus::Update { update, freshness },
        None => CachedStatus::UpToDate(freshness),
    }
}

fn update_from_cache(cache: &cache::UpdateCache, latest: &str) -> Option<AvailableUpdate> {
    if !version::is_newer(latest, current_version()) {
        return None;
    }
    Some(AvailableUpdate {
        version: latest.to_string(),
        released: cache.released.clone(),
        // The cache stores the manifest's own URL; the install-source anchor is
        // applied on read so a cached result opens the same page a fresh check
        // would.
        update_url: manifest::update_url_for_install_source(&trusted_or_default(
            cache.update_url.as_deref(),
            DEFAULT_UPDATE_URL,
        )),
        notes_url: trusted_or_default(cache.notes_url.as_deref(), DEFAULT_NOTES_URL),
    })
}

/// Cached URLs are re-validated on read: the cache file is user-writable, and
/// these strings end up in `xdg-open`.
fn trusted_or_default(candidate: Option<&str>, fallback: &str) -> String {
    candidate
        .filter(|url| manifest::is_trusted_url(url))
        .unwrap_or(fallback)
        .to_string()
}

/// Fetch the manifest and record the result. Ignores the throttle: callers use
/// this for explicit, user-initiated checks.
pub fn check_now(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    cache_store: &UpdateCacheStore,
) -> Result<CheckOutcome, String> {
    if compiled_out() {
        return Err(COMPILED_OUT_MESSAGE.to_string());
    }

    let result = fetch::fetch(process_broker, MANIFEST_URL, FETCH_TIMEOUT)
        .and_then(|body| manifest::parse_manifest(&body));

    // Every attempt is persisted, including a failed explicit one: this process
    // has already made the request, and the daemon reads the same file to decide
    // whether another is due.
    record_attempt(cache_store, result.as_ref().ok());

    result.map(|manifest| outcome_for(&manifest))
}

/// Persist one attempt. A manifest also refreshes the stored result and the
/// success stamp; a failure moves the attempt stamp alone, leaving the previous
/// result in place but no longer claiming it was just verified.
fn record_attempt(cache_store: &UpdateCacheStore, manifest: Option<&manifest::ReleaseManifest>) {
    let now = cache::now_unix();
    let _ = cache_store.update(|cache| {
        cache.last_attempt_unix = now;
        cache.last_attempt_outcome = Some(if manifest.is_some() {
            cache::AttemptOutcome::Succeeded
        } else {
            cache::AttemptOutcome::Failed
        });
        if let Some(manifest) = manifest {
            cache.last_success_unix = now;
            cache.latest_version = Some(manifest.version.clone());
            cache.released = manifest.released.clone();
            cache.update_url = Some(manifest.update_url.clone());
            cache.notes_url = Some(manifest.notes_url.clone());
        }
    });
}

fn outcome_for(manifest: &manifest::ReleaseManifest) -> CheckOutcome {
    if version::is_newer(&manifest.version, current_version()) {
        CheckOutcome::Update(AvailableUpdate {
            version: manifest.version.clone(),
            released: manifest.released.clone(),
            update_url: manifest::update_url_for_install_source(&manifest.update_url),
            notes_url: manifest.notes_url.clone(),
        })
    } else {
        CheckOutcome::UpToDate {
            latest: manifest.version.clone(),
        }
    }
}

/// Watcher-owned monotonic throttle for caches that cannot be written.
///
/// The daemon owns one instance in its update-watch loop. Keeping this state
/// there avoids ambient process-wide synchronization; explicit About/CLI checks
/// intentionally bypass the interval and therefore do not need it.
#[derive(Debug)]
pub(crate) struct CheckThrottle {
    cache_store: UpdateCacheStore,
    last_attempt: Option<std::time::Instant>,
}

impl CheckThrottle {
    pub(crate) fn new(cache_store: UpdateCacheStore) -> Self {
        Self {
            cache_store,
            last_attempt: None,
        }
    }

    /// Run a check only when both the process-local and persisted attempts are
    /// older than `interval`. Returns `None` when skipped or failed.
    pub(crate) fn check_if_due(
        &mut self,
        process_broker: &crate::process_broker::ProcessBrokerHandle,
        interval: Duration,
    ) -> Option<CheckOutcome> {
        if !self.is_due_now(interval) {
            return None;
        }

        // Record before fetching, so an unwritable cache still cannot turn one
        // request into another request on every watcher wakeup.
        self.note_attempt();
        match check_now(process_broker, &self.cache_store) {
            Ok(outcome) => Some(outcome),
            Err(err) => {
                // `check_now` already tried to persist the attempt for other
                // processes; this owned throttle covers persistence failure.
                debug!("Update check failed: {err}");
                None
            }
        }
    }

    fn is_due_now(&self, interval: Duration) -> bool {
        if self.attempted_within(interval) {
            return false;
        }
        is_due(
            self.cache_store.load().last_attempt_unix,
            cache::now_unix(),
            interval,
        )
    }

    fn note_attempt(&mut self) {
        self.last_attempt = Some(std::time::Instant::now());
    }

    fn attempted_within(&self, interval: Duration) -> bool {
        self.last_attempt.is_some_and(|at| at.elapsed() < interval)
    }
}

/// Whether a check is due, given the last attempt and the current time.
fn is_due(last_attempt_unix: u64, now_unix: u64, interval: Duration) -> bool {
    if last_attempt_unix == 0 {
        return true;
    }
    // A clock that jumped backwards makes the stored stamp look like the
    // future; treat that as due rather than never checking again.
    if last_attempt_unix > now_unix {
        return true;
    }
    now_unix - last_attempt_unix >= interval.as_secs()
}

/// Whether the user has yet to be told about `update`.
pub(crate) fn notification_pending(
    cache_store: &UpdateCacheStore,
    update: &AvailableUpdate,
) -> bool {
    cache_store.load().notified_version.as_deref() != Some(update.version.as_str())
}

/// Record that the user has been told about `update`, so the notification is
/// not repeated on every daemon start. Returns `false` when it had already been
/// claimed.
///
/// Callers claim *after* a successful delivery: claiming first would swallow the
/// only notification for a release if the notification daemon was not up yet.
pub(crate) fn claim_notification(cache_store: &UpdateCacheStore, update: &AvailableUpdate) -> bool {
    cache_store
        .update(|cache| {
            if cache.notified_version.as_deref() == Some(update.version.as_str()) {
                return false;
            }
            cache.notified_version = Some(update.version.clone());
            true
        })
        .unwrap_or(false)
}

/// Update instructions for this build's install source.
pub fn update_instructions_url() -> String {
    manifest::update_url_for_install_source(DEFAULT_UPDATE_URL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_store(root: &std::path::Path) -> UpdateCacheStore {
        UpdateCacheStore::at_path(root.join("update-check.json"))
    }

    #[test]
    fn env_opt_out_recognizes_falsey_values() {
        assert!(env_value_opts_out(Some(std::ffi::OsStr::new("1"))));
        assert!(!env_value_opts_out(Some(std::ffi::OsStr::new("0"))));

        for falsey in ["false", "off", "no", "disable", "disabled", "  OFF  ", ""] {
            assert!(
                !env_value_opts_out(Some(std::ffi::OsStr::new(falsey))),
                "expected {falsey:?} to leave checks on"
            );
        }

        // An unrecognized value still opts out: this switch exists to stop
        // network access, so it is honored rather than ignored.
        assert!(env_value_opts_out(Some(std::ffi::OsStr::new("please"))));
        assert!(!env_value_opts_out(None));
    }

    #[test]
    fn a_compiled_out_build_refuses_to_fetch() {
        if !compiled_out() {
            return;
        }

        assert!(!background_checks_enabled(true));
        let process_broker = crate::process_broker::start_for_runtime()
            .expect("test starts its explicit process broker owner");
        let dir = crate::test_temp::tempdir().expect("isolated update cache fixture");
        let store = cache_store(dir.path());
        assert_eq!(
            check_now(&process_broker.handle(), &store),
            Err(COMPILED_OUT_MESSAGE.to_string())
        );
    }

    #[test]
    fn due_when_never_attempted_or_interval_elapsed() {
        let day = Duration::from_secs(86_400);

        assert!(is_due(0, 1_750_000_000, day));
        assert!(is_due(1_750_000_000, 1_750_086_400, day));
        assert!(!is_due(1_750_000_000, 1_750_086_399, day));
        // Clock moved backwards.
        assert!(is_due(1_750_086_400, 1_750_000_000, day));
    }

    /// The in-process guard is what stops a cache that cannot be written from
    /// turning a daily check into one request per daemon wakeup.
    #[test]
    fn each_watcher_owns_its_in_process_attempt_throttle() {
        let dir = crate::test_temp::tempdir()
            .expect("watcher-throttle fixture creates an isolated cache directory");
        let store = cache_store(dir.path());

        let mut attempted = CheckThrottle::new(store.clone());
        attempted.note_attempt();
        let untouched = CheckThrottle::new(store);

        assert!(attempted.attempted_within(Duration::from_secs(86_400)));
        // A zero interval means "no throttle", so the guard must not block.
        assert!(!attempted.attempted_within(Duration::ZERO));

        // One watcher's attempt does not become ambient process-wide state.
        assert!(!attempted.is_due_now(Duration::from_secs(86_400)));
        assert!(untouched.is_due_now(Duration::from_secs(86_400)));
    }

    #[test]
    fn outcome_reflects_version_ordering() {
        let older = manifest::ReleaseManifest {
            version: "0.0.1".to_string(),
            released: None,
            notes_url: DEFAULT_NOTES_URL.to_string(),
            update_url: DEFAULT_UPDATE_URL.to_string(),
        };
        assert!(matches!(outcome_for(&older), CheckOutcome::UpToDate { .. }));

        let newer = manifest::ReleaseManifest {
            version: "999.0.0".to_string(),
            released: Some("2030-01-01".to_string()),
            notes_url: DEFAULT_NOTES_URL.to_string(),
            update_url: DEFAULT_UPDATE_URL.to_string(),
        };
        let update = match outcome_for(&newer) {
            CheckOutcome::Update(update) => Some(update),
            CheckOutcome::UpToDate { .. } => None,
        }
        .expect("newer-version fixture produces an update outcome");
        assert_eq!(update.version, "999.0.0");
        assert!(update.update_url.starts_with(DEFAULT_UPDATE_URL));
    }

    /// The About window's whole data path: what a completed check wrote is what
    /// the dialog reads back, with the install-source anchor applied.
    #[test]
    fn cached_status_reports_what_the_last_check_stored() {
        let dir = crate::test_temp::tempdir()
            .expect("cached-status fixture creates an isolated cache directory");
        let store = cache_store(dir.path());

        assert_eq!(
            cached_status(&store),
            CachedStatus::Never(Freshness::default())
        );

        let now = cache::now_unix();
        let mut seeded = store.load();
        seeded.last_attempt_unix = now;
        seeded.last_success_unix = now;
        seeded.latest_version = Some("999.0.0".to_string());
        seeded.released = Some("2030-01-01".to_string());
        seeded.update_url = Some(DEFAULT_UPDATE_URL.to_string());
        seeded.notes_url = Some(DEFAULT_NOTES_URL.to_string());
        store.store(&seeded);
        let update = match cached_status(&store) {
            CachedStatus::Update { update, .. } => Some(update),
            CachedStatus::Never(_) | CachedStatus::UpToDate(_) => None,
        }
        .expect("seeded cache fixture reports its available update");
        assert_eq!(update.version, "999.0.0");
        assert_eq!(update.released.as_deref(), Some("2030-01-01"));
        assert_eq!(
            update.update_url,
            manifest::update_url_for_install_source(DEFAULT_UPDATE_URL)
        );
        assert!(notification_pending(&store, &update));
        assert!(claim_notification(&store, &update));
        assert!(!notification_pending(&store, &update));

        // A latest version that is not newer than this build reads as up to date.
        let mut cache = store.load();
        cache.latest_version = Some("0.0.1".to_string());
        store.store(&cache);
        assert!(matches!(
            cached_status(&store),
            CachedStatus::UpToDate(Freshness {
                checked_seconds_ago: Some(_),
                last_attempt_failed: false,
            })
        ));

        // The sequence that actually happens: a success, then a later failed
        // background check. The age still refers to the success — but the failed
        // retry is reported with it, so nothing prints a bare "Checked N ago"
        // for an attempt that failed seconds ago.
        let mut cache = store.load();
        cache.last_success_unix = cache::now_unix() - 7_200;
        cache.last_attempt_unix = cache::now_unix();
        cache.last_attempt_outcome = Some(cache::AttemptOutcome::Failed);
        store.store(&cache);
        let freshness = match cached_status(&store) {
            CachedStatus::UpToDate(freshness) => Some(freshness),
            CachedStatus::Never(_) | CachedStatus::Update { .. } => None,
        }
        .expect("older-version cache fixture reports an up-to-date verdict");
        assert!(freshness.last_attempt_failed);
        assert!(
            freshness
                .checked_seconds_ago
                .is_some_and(|age| age >= 7_200)
        );

        // ...and the attempt the failure recorded is what suppresses the next
        // check, so a failing server is not retried on every wakeup.
        assert!(!is_due(
            store.load().last_attempt_unix,
            cache::now_unix(),
            Duration::from_secs(86_400)
        ));

        // A record with no success on it (a migrated v1 file) is unknown rather
        // than known-failed.
        let mut cache = store.load();
        cache.last_success_unix = 0;
        cache.last_attempt_outcome = None;
        store.store(&cache);
        assert_eq!(
            cached_status(&store),
            CachedStatus::UpToDate(Freshness {
                checked_seconds_ago: None,
                last_attempt_failed: false,
            })
        );
    }

    #[test]
    fn a_first_persisted_failure_remains_visible_after_reopening_about() {
        let dir = crate::test_temp::tempdir()
            .expect("first-failure fixture creates an isolated cache directory");
        let store = cache_store(dir.path());

        record_attempt(&store, None);

        assert_eq!(
            cached_status(&store),
            CachedStatus::Never(Freshness {
                checked_seconds_ago: None,
                last_attempt_failed: true,
            })
        );
    }

    #[test]
    fn a_same_second_failure_is_not_mistaken_for_the_previous_success() {
        let dir = crate::test_temp::tempdir()
            .expect("same-second fixture creates an isolated cache directory");
        let store = cache_store(dir.path());

        let stamp = cache::now_unix();
        let mut seeded = store.load();
        seeded.last_attempt_unix = stamp;
        seeded.last_success_unix = stamp;
        seeded.last_attempt_outcome = Some(cache::AttemptOutcome::Failed);
        seeded.latest_version = Some("0.0.1".to_string());
        store.store(&seeded);

        assert!(matches!(
            cached_status(&store),
            CachedStatus::UpToDate(Freshness {
                checked_seconds_ago: Some(_),
                last_attempt_failed: true,
            })
        ));
    }

    #[test]
    fn cached_urls_are_revalidated_before_use() {
        assert_eq!(
            trusted_or_default(Some("https://evil.example/x"), DEFAULT_UPDATE_URL),
            DEFAULT_UPDATE_URL
        );
        assert_eq!(
            trusted_or_default(
                Some("https://wayscriber.com/docs/x#apt"),
                DEFAULT_UPDATE_URL
            ),
            "https://wayscriber.com/docs/x#apt"
        );
        assert_eq!(
            trusted_or_default(None, DEFAULT_UPDATE_URL),
            DEFAULT_UPDATE_URL
        );
    }
}
