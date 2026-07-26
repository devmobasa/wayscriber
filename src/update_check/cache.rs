//! On-disk record of the last update check.
//!
//! Lives in the XDG cache directory because it is regenerable state, not user
//! data: deleting it costs one extra HTTP request. It holds no identifiers —
//! just the last result, when it was fetched, and which version the user has
//! already been told about (so a notification fires once per release, not once
//! per login).

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use log::debug;
use serde::{Deserialize, Serialize};

use crate::durable_io::{AtomicWriteOptions, OverwriteMode, PermissionPolicy, SymlinkPolicy};

/// Schema version, so a future format change can be detected rather than
/// misread. v2 split the single "checked" timestamp in two; v3 records the
/// attempt outcome directly instead of inferring failure from second-resolution
/// wall-clock timestamps.
const CACHE_VERSION: u32 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UpdateCacheStore {
    path: PathBuf,
}

impl UpdateCacheStore {
    pub(crate) fn from_resolver(
        paths: &crate::paths::PathResolver,
    ) -> Result<Self, crate::paths::PathResolutionError> {
        Ok(Self {
            path: paths.update_check_cache_file()?,
        })
    }

    #[cfg(test)]
    pub(crate) fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub(crate) fn load(&self) -> UpdateCache {
        load_from(&self.path)
    }

    pub(crate) fn update<T>(&self, mutate: impl FnOnce(&mut UpdateCache) -> T) -> Option<T> {
        let _guard = CacheLock::acquire(&self.path)?;
        let mut cache = load_from(&self.path);
        let result = mutate(&mut cache);
        store_at(&self.path, &cache).then_some(result)
    }

    #[cfg(test)]
    pub(crate) fn store(&self, cache: &UpdateCache) {
        let _ = store_at(&self.path, cache);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttemptOutcome {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpdateCache {
    #[serde(default)]
    pub(crate) version: u32,
    /// Unix seconds of the last attempt, successful or not. Throttling reads
    /// this, so a server that is down cannot be retried on every wakeup.
    #[serde(default)]
    pub(crate) last_attempt_unix: u64,
    /// Unix seconds of the last attempt that actually returned a manifest.
    /// Freshness ("checked 2 hours ago") reads this, so a failed check can never
    /// make a stale result look verified.
    #[serde(default)]
    pub(crate) last_success_unix: u64,
    /// Outcome of the latest attempt. `None` is intentional for migrated cache
    /// records whose older schema cannot distinguish every sequence reliably.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_attempt_outcome: Option<AttemptOutcome>,
    /// v1 stored one timestamp for both meanings. Read for migration only;
    /// never written back.
    #[serde(rename = "last_checked_unix", default, skip_serializing)]
    legacy_last_checked_unix: u64,
    /// Newest version seen on the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) latest_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) released: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) update_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) notes_url: Option<String>,
    /// Version the user was already notified about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) notified_version: Option<String>,
}

/// Advisory lock guarding read-modify-write cycles.
fn lock_path(cache_path: &Path) -> PathBuf {
    cache_path.with_extension("lock")
}

/// Attempts to take the lock for up to half a second. Nothing that takes this
/// lock is on a paint path: callers have either just finished a network request
/// or are on the daemon's watcher thread. On timeout the mutation is skipped;
/// it must never fall back to an unlocked read-modify-write.
const LOCK_ATTEMPTS: u32 = 25;
const LOCK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(20);

/// Read-modify-write the cache under an advisory lock.
///
/// The daemon, the About dialog, and `--check-update` are separate processes
/// sharing one file. Without this, a notification claim and a stored check
/// result can each be written from a snapshot taken before the other, silently
/// erasing `notified_version` (a second notification for the same release) or
/// restoring a superseded result.
///
/// Whole-file writes are atomic, so readers never need the lock; only these
/// cycles do. If the lock cannot be taken promptly, the mutation is skipped:
/// writing an unlocked snapshot could erase a newer attempt or notification
/// claim from another process.
/// Holds the advisory lock for as long as it is in scope.
struct CacheLock(std::fs::File);

impl CacheLock {
    fn acquire(cache_path: &Path) -> Option<Self> {
        let parent = cache_path.parent()?;
        if let Err(err) = std::fs::create_dir_all(parent) {
            debug!("Failed to create update cache dir for locking: {err}");
            return None;
        }

        let path = lock_path(cache_path);
        let file = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
        {
            Ok(file) => file,
            Err(err) => {
                debug!("Failed to open update cache lock {}: {err}", path.display());
                return None;
            }
        };

        for attempt in 0..LOCK_ATTEMPTS {
            if crate::session::try_lock_exclusive(&file).is_ok() {
                return Some(Self(file));
            }
            if attempt + 1 < LOCK_ATTEMPTS {
                std::thread::sleep(LOCK_RETRY_DELAY);
            }
        }

        debug!("Skipping update cache mutation; another process holds the lock");
        None
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        if let Err(err) = crate::session::unlock(&self.0) {
            debug!("Failed to release the update cache lock: {err}");
        }
    }
}

/// Read the cache, degrading to an empty record on any problem: a corrupt or
/// unreadable cache must never block startup or the About window.
pub(crate) fn load_from(path: &Path) -> UpdateCache {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return UpdateCache::default();
    };
    match serde_json::from_str::<UpdateCache>(&raw) {
        Ok(cache) if cache.version <= CACHE_VERSION => migrated(cache),
        Ok(cache) => {
            debug!(
                "Ignoring update cache written by a newer version (schema {})",
                cache.version
            );
            UpdateCache::default()
        }
        Err(err) => {
            debug!("Ignoring unreadable update cache: {err}");
            UpdateCache::default()
        }
    }
}

/// Fold an older cache record into the current shape.
///
/// v1's single timestamp counted failed attempts too, so it is carried over as
/// an *attempt* only: promoting it to a success would reintroduce exactly the
/// "verified just now" claim that splitting the two timestamps removes. The
/// throttle therefore survives the upgrade, while freshness reads as unknown
/// until the next successful check.
fn migrated(mut cache: UpdateCache) -> UpdateCache {
    if cache.last_attempt_unix == 0 {
        cache.last_attempt_unix = cache.legacy_last_checked_unix;
    }
    cache.legacy_last_checked_unix = 0;
    cache
}

/// Overwrite the cache outright. Test-only: production writes go through
/// [`update`] so the read-modify-write cycle stays under the lock.
/// Failures are logged at debug and reported to [`update`]. The watcher keeps
/// its own process-local attempt time, so an unwritable cache still cannot turn
/// periodic throttling off.
fn store_at(path: &Path, cache: &UpdateCache) -> bool {
    let mut cache = cache.clone();
    cache.version = CACHE_VERSION;

    let Ok(contents) = serde_json::to_string_pretty(&cache) else {
        debug!("Failed to serialize update cache");
        return false;
    };

    if let Some(parent) = path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        debug!(
            "Failed to create update cache dir {}: {err}",
            parent.display()
        );
        return false;
    }

    if let Err(err) = crate::durable_io::write_text_atomic(
        path,
        &contents,
        AtomicWriteOptions {
            overwrite: OverwriteMode::Replace,
            permissions: PermissionPolicy::PreserveExistingOrMode(0o644),
            symlink: SymlinkPolicy::Reject,
            sync_file: false,
            sync_parent: false,
        },
    ) {
        debug!("Failed to write update cache {}: {err}", path.display());
        return false;
    }
    true
}

/// Current wall-clock time in Unix seconds (0 before the epoch, which only
/// happens on a badly misconfigured clock).
pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Seconds since `then`, saturating at 0 when the clock has moved backwards.
pub(crate) fn seconds_since(then: u64) -> Option<u64> {
    if then == 0 {
        return None;
    }
    Some(now_unix().saturating_sub(then))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_temp::tempdir;

    fn sample() -> UpdateCache {
        UpdateCache {
            version: CACHE_VERSION,
            last_attempt_unix: 1_750_000_000,
            last_success_unix: 1_750_000_000,
            last_attempt_outcome: Some(AttemptOutcome::Succeeded),
            legacy_last_checked_unix: 0,
            latest_version: Some("0.9.23".to_string()),
            released: Some("2026-07-20".to_string()),
            update_url: Some(
                "https://wayscriber.com/docs/getting-started/updating.html".to_string(),
            ),
            notes_url: Some("https://wayscriber.com/docs/release-notes.html".to_string()),
            notified_version: None,
        }
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("update-check.json");

        store_at(&path, &sample());

        assert_eq!(load_from(&path), sample());
    }

    #[test]
    fn missing_or_corrupt_cache_reads_as_empty() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("absent.json");
        assert_eq!(load_from(&missing), UpdateCache::default());

        let corrupt = dir.path().join("corrupt.json");
        std::fs::write(&corrupt, "{not json").unwrap();
        assert_eq!(load_from(&corrupt), UpdateCache::default());
    }

    #[test]
    fn a_v1_cache_keeps_its_throttle_but_not_a_claim_of_freshness() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v1.json");
        std::fs::write(
            &path,
            r#"{"version": 1, "last_checked_unix": 1750000000, "latest_version": "0.9.23"}"#,
        )
        .unwrap();

        let cache = load_from(&path);

        assert_eq!(cache.last_attempt_unix, 1_750_000_000);
        // Unknown, not "just verified": the v1 stamp may have been a failure.
        assert_eq!(cache.last_success_unix, 0);
        assert_eq!(cache.last_attempt_outcome, None);
        assert_eq!(cache.latest_version.as_deref(), Some("0.9.23"));

        // Rewriting drops the legacy key entirely.
        store_at(&path, &cache);
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("last_checked_unix"), "{raw}");
        assert!(raw.contains("last_attempt_unix"), "{raw}");
    }

    #[test]
    fn a_v2_cache_does_not_guess_an_attempt_outcome_from_timestamps() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v2.json");
        std::fs::write(
            &path,
            r#"{
                "version": 2,
                "last_attempt_unix": 1750007200,
                "last_success_unix": 1750000000,
                "latest_version": "0.9.23"
            }"#,
        )
        .unwrap();

        let cache = load_from(&path);

        assert_eq!(cache.last_attempt_unix, 1_750_007_200);
        assert_eq!(cache.last_success_unix, 1_750_000_000);
        assert_eq!(cache.last_attempt_outcome, None);
    }

    /// Concurrent read-modify-write cycles must not lose each other's fields.
    /// Without the lock, the thread that loads first and stores last silently
    /// reverts the other — which in practice means a re-sent notification or a
    /// restored stale result.
    #[test]
    fn concurrent_updates_do_not_erase_each_other() {
        let dir = tempdir().unwrap();
        let store = UpdateCacheStore::at_path(dir.path().join("update-check.json"));

        let mut initial = sample();
        initial.notified_version = None;
        initial.last_attempt_unix = 0;
        store.store(&initial);

        // The slow writer holds the lock across a wide load→store window; the
        // fast one starts inside that window. Unlocked, the slow store would
        // land last and revert the fast writer's field.
        let (entered_tx, entered_rx) = std::sync::mpsc::sync_channel(0);
        let (continue_tx, continue_rx) = std::sync::mpsc::sync_channel(0);
        let slow_store = store.clone();
        let slow = std::thread::spawn(move || {
            let _ = slow_store.update(|cache| {
                cache.notified_version = Some("0.9.23".to_string());
                entered_tx.send(()).unwrap();
                continue_rx.recv().unwrap();
            });
        });
        entered_rx.recv().unwrap();
        let fast_store = store.clone();
        let fast = std::thread::spawn(move || {
            let _ = fast_store.update(|cache| cache.last_attempt_unix = 1_800_000_000);
        });
        continue_tx.send(()).unwrap();
        slow.join().unwrap();
        fast.join().unwrap();

        let final_cache = store.load();
        assert_eq!(
            final_cache.notified_version.as_deref(),
            Some("0.9.23"),
            "the notification claim was lost"
        );
        assert_eq!(
            final_cache.last_attempt_unix, 1_800_000_000,
            "the recorded attempt was reverted by a concurrent writer"
        );
        // Fields neither writer touched survive both cycles.
        assert_eq!(final_cache.latest_version.as_deref(), Some("0.9.23"));
    }

    #[test]
    fn a_timed_out_lock_does_not_fall_back_to_an_unlocked_write() {
        let dir = tempdir().unwrap();
        let store = UpdateCacheStore::at_path(dir.path().join("update-check.json"));

        let initial = sample();
        store.store(&initial);
        let held = CacheLock::acquire(&store.path).expect("first writer should acquire the lock");

        let update_result = store.update(|cache| {
            cache.last_attempt_unix = 1_800_000_000;
        });

        assert_eq!(
            update_result, None,
            "the contending mutation must be skipped"
        );
        assert_eq!(
            store.load(),
            initial,
            "the cache must not be written without the lock"
        );
        drop(held);
    }

    #[cfg(unix)]
    #[test]
    fn a_rejected_cache_write_is_not_reported_as_persisted() {
        let dir = tempdir().unwrap();
        let store = UpdateCacheStore::at_path(dir.path().join("update-check.json"));

        let path = store.path.clone();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(dir.path().join("redirected.json"), &path).unwrap();

        let update_result = store.update(|cache| {
            cache.last_attempt_unix = 1_800_000_000;
        });

        assert_eq!(update_result, None, "a rejected write was not persisted");
        assert!(!dir.path().join("redirected.json").exists());
    }

    #[test]
    fn cache_from_a_newer_schema_is_ignored() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("future.json");
        std::fs::write(&path, r#"{"version": 99, "latest_version": "9.9.9"}"#).unwrap();

        assert_eq!(load_from(&path), UpdateCache::default());
    }

    #[test]
    fn seconds_since_handles_unset_and_future_timestamps() {
        assert_eq!(seconds_since(0), None);
        assert_eq!(seconds_since(now_unix() + 3600), Some(0));
        assert!(seconds_since(now_unix() - 60).unwrap() >= 60);
    }
}
