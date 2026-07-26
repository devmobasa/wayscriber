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
    std::env::var_os(DISABLE_UPDATE_CHECK_ENV)
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
pub fn cached_status() -> CachedStatus {
    let cache = cache::load();
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
pub fn check_now() -> Result<CheckOutcome, String> {
    if compiled_out() {
        return Err(COMPILED_OUT_MESSAGE.to_string());
    }

    let result =
        fetch::fetch(MANIFEST_URL, FETCH_TIMEOUT).and_then(|body| manifest::parse_manifest(&body));

    // Every attempt is persisted, including a failed explicit one: this process
    // has already made the request, and the daemon reads the same file to decide
    // whether another is due.
    record_attempt(result.as_ref().ok());

    result.map(|manifest| outcome_for(&manifest))
}

/// Persist one attempt. A manifest also refreshes the stored result and the
/// success stamp; a failure moves the attempt stamp alone, leaving the previous
/// result in place but no longer claiming it was just verified.
fn record_attempt(manifest: Option<&manifest::ReleaseManifest>) {
    let now = cache::now_unix();
    let _ = cache::update(|cache| {
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
#[derive(Debug, Default)]
pub(crate) struct CheckThrottle {
    last_attempt: Option<std::time::Instant>,
}

impl CheckThrottle {
    /// Run a check only when both the process-local and persisted attempts are
    /// older than `interval`. Returns `None` when skipped or failed.
    pub(crate) fn check_if_due(&mut self, interval: Duration) -> Option<CheckOutcome> {
        if !self.is_due_now(interval) {
            return None;
        }

        // Record before fetching, so an unwritable cache still cannot turn one
        // request into another request on every watcher wakeup.
        self.note_attempt();
        match check_now() {
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
        is_due(cache::load().last_attempt_unix, cache::now_unix(), interval)
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
pub fn notification_pending(update: &AvailableUpdate) -> bool {
    cache::load().notified_version.as_deref() != Some(update.version.as_str())
}

/// Record that the user has been told about `update`, so the notification is
/// not repeated on every daemon start. Returns `false` when it had already been
/// claimed.
///
/// Callers claim *after* a successful delivery: claiming first would swallow the
/// only notification for a release if the notification daemon was not up yet.
pub fn claim_notification(update: &AvailableUpdate) -> bool {
    cache::update(|cache| {
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

    /// Restores the opt-out variable when the test ends, however it ends.
    struct OptOutEnv(Option<std::ffi::OsString>);

    impl OptOutEnv {
        fn capture() -> Self {
            Self(std::env::var_os(DISABLE_UPDATE_CHECK_ENV))
        }

        fn set(&self, value: &str) {
            unsafe { std::env::set_var(DISABLE_UPDATE_CHECK_ENV, value) };
        }

        fn clear(&self) {
            unsafe { std::env::remove_var(DISABLE_UPDATE_CHECK_ENV) };
        }
    }

    impl Drop for OptOutEnv {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => unsafe { std::env::set_var(DISABLE_UPDATE_CHECK_ENV, value) },
                None => unsafe { std::env::remove_var(DISABLE_UPDATE_CHECK_ENV) },
            }
        }
    }

    #[test]
    fn env_opt_out_recognizes_falsey_values() {
        let _lock = crate::test_env::lock();
        let env = OptOutEnv::capture();

        env.set("1");
        assert!(env_opt_out());
        assert!(!background_checks_enabled(true));

        env.set("0");
        assert!(!env_opt_out());
        // A build with the check compiled out stays off regardless.
        assert_eq!(background_checks_enabled(true), !compiled_out());

        for falsey in ["false", "off", "no", "disable", "disabled", "  OFF  ", ""] {
            env.set(falsey);
            assert!(!env_opt_out(), "expected {falsey:?} to leave checks on");
        }

        // An unrecognized value still opts out: this switch exists to stop
        // network access, so it is honored rather than ignored.
        env.set("please");
        assert!(env_opt_out());

        env.clear();
        assert!(!env_opt_out());
        assert!(!background_checks_enabled(false));
    }

    #[test]
    fn a_compiled_out_build_refuses_to_fetch() {
        if !compiled_out() {
            return;
        }

        assert!(!background_checks_enabled(true));
        assert_eq!(check_now(), Err(COMPILED_OUT_MESSAGE.to_string()));
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
        let _lock = crate::test_env::lock();
        let dir = crate::test_temp::tempdir().unwrap();
        let previous = std::env::var_os(crate::env_vars::XDG_CACHE_HOME_ENV);
        unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, dir.path()) };

        let mut attempted = CheckThrottle::default();
        attempted.note_attempt();
        let untouched = CheckThrottle::default();

        assert!(attempted.attempted_within(Duration::from_secs(86_400)));
        // A zero interval means "no throttle", so the guard must not block.
        assert!(!attempted.attempted_within(Duration::ZERO));

        // One watcher's attempt does not become ambient process-wide state.
        assert!(!attempted.is_due_now(Duration::from_secs(86_400)));
        assert!(untouched.is_due_now(Duration::from_secs(86_400)));

        match previous {
            Some(value) => unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, value) },
            None => unsafe { std::env::remove_var(crate::env_vars::XDG_CACHE_HOME_ENV) },
        }
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
        match outcome_for(&newer) {
            CheckOutcome::Update(update) => {
                assert_eq!(update.version, "999.0.0");
                assert!(update.update_url.starts_with(DEFAULT_UPDATE_URL));
            }
            other => panic!("expected an update, got {other:?}"),
        }
    }

    /// The About window's whole data path: what a completed check wrote is what
    /// the dialog reads back, with the install-source anchor applied.
    #[test]
    fn cached_status_reports_what_the_last_check_stored() {
        let _lock = crate::test_env::lock();
        let dir = crate::test_temp::tempdir().unwrap();
        let previous = std::env::var_os(crate::env_vars::XDG_CACHE_HOME_ENV);
        unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, dir.path()) };

        assert_eq!(cached_status(), CachedStatus::Never(Freshness::default()));

        let now = cache::now_unix();
        let mut seeded = cache::load();
        seeded.last_attempt_unix = now;
        seeded.last_success_unix = now;
        seeded.latest_version = Some("999.0.0".to_string());
        seeded.released = Some("2030-01-01".to_string());
        seeded.update_url = Some(DEFAULT_UPDATE_URL.to_string());
        seeded.notes_url = Some(DEFAULT_NOTES_URL.to_string());
        cache::store(&seeded);
        match cached_status() {
            CachedStatus::Update { update, .. } => {
                assert_eq!(update.version, "999.0.0");
                assert_eq!(update.released.as_deref(), Some("2030-01-01"));
                assert_eq!(
                    update.update_url,
                    manifest::update_url_for_install_source(DEFAULT_UPDATE_URL)
                );
                assert!(notification_pending(&update));
                assert!(claim_notification(&update));
                assert!(!notification_pending(&update));
            }
            other => panic!("expected a cached update, got {other:?}"),
        }

        // A latest version that is not newer than this build reads as up to date.
        let mut cache = cache::load();
        cache.latest_version = Some("0.0.1".to_string());
        cache::store(&cache);
        assert!(matches!(
            cached_status(),
            CachedStatus::UpToDate(Freshness {
                checked_seconds_ago: Some(_),
                last_attempt_failed: false,
            })
        ));

        // The sequence that actually happens: a success, then a later failed
        // background check. The age still refers to the success — but the failed
        // retry is reported with it, so nothing prints a bare "Checked N ago"
        // for an attempt that failed seconds ago.
        let mut cache = cache::load();
        cache.last_success_unix = cache::now_unix() - 7_200;
        cache.last_attempt_unix = cache::now_unix();
        cache.last_attempt_outcome = Some(cache::AttemptOutcome::Failed);
        cache::store(&cache);
        let CachedStatus::UpToDate(freshness) = cached_status() else {
            panic!("expected an up-to-date verdict");
        };
        assert!(freshness.last_attempt_failed);
        assert!(
            freshness
                .checked_seconds_ago
                .is_some_and(|age| age >= 7_200)
        );

        // ...and the attempt the failure recorded is what suppresses the next
        // check, so a failing server is not retried on every wakeup.
        assert!(!is_due(
            cache::load().last_attempt_unix,
            cache::now_unix(),
            Duration::from_secs(86_400)
        ));

        // A record with no success on it (a migrated v1 file) is unknown rather
        // than known-failed.
        let mut cache = cache::load();
        cache.last_success_unix = 0;
        cache.last_attempt_outcome = None;
        cache::store(&cache);
        assert_eq!(
            cached_status(),
            CachedStatus::UpToDate(Freshness {
                checked_seconds_ago: None,
                last_attempt_failed: false,
            })
        );

        match previous {
            Some(value) => unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, value) },
            None => unsafe { std::env::remove_var(crate::env_vars::XDG_CACHE_HOME_ENV) },
        }
    }

    #[test]
    fn a_first_persisted_failure_remains_visible_after_reopening_about() {
        let _lock = crate::test_env::lock();
        let dir = crate::test_temp::tempdir().unwrap();
        let previous = std::env::var_os(crate::env_vars::XDG_CACHE_HOME_ENV);
        unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, dir.path()) };

        record_attempt(None);

        assert_eq!(
            cached_status(),
            CachedStatus::Never(Freshness {
                checked_seconds_ago: None,
                last_attempt_failed: true,
            })
        );

        match previous {
            Some(value) => unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, value) },
            None => unsafe { std::env::remove_var(crate::env_vars::XDG_CACHE_HOME_ENV) },
        }
    }

    #[test]
    fn a_same_second_failure_is_not_mistaken_for_the_previous_success() {
        let _lock = crate::test_env::lock();
        let dir = crate::test_temp::tempdir().unwrap();
        let previous = std::env::var_os(crate::env_vars::XDG_CACHE_HOME_ENV);
        unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, dir.path()) };

        let stamp = cache::now_unix();
        let mut seeded = cache::load();
        seeded.last_attempt_unix = stamp;
        seeded.last_success_unix = stamp;
        seeded.last_attempt_outcome = Some(cache::AttemptOutcome::Failed);
        seeded.latest_version = Some("0.0.1".to_string());
        cache::store(&seeded);

        assert!(matches!(
            cached_status(),
            CachedStatus::UpToDate(Freshness {
                checked_seconds_ago: Some(_),
                last_attempt_failed: true,
            })
        ));

        match previous {
            Some(value) => unsafe { std::env::set_var(crate::env_vars::XDG_CACHE_HOME_ENV, value) },
            None => unsafe { std::env::remove_var(crate::env_vars::XDG_CACHE_HOME_ENV) },
        }
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
