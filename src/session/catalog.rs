use anyhow::{Context, Result, anyhow};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::lock::{lock_exclusive, open_runtime_lock_file, unlock};
use super::options::SessionOptions;

const CATALOG_VERSION: u32 = 1;
static NEXT_CATALOG_ID: AtomicU64 = AtomicU64::new(0);
static NEXT_CATALOG_TEMP_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogEvent {
    Opened,
    Saved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub canonical_path: Option<String>,
    pub created_at_millis: u64,
    pub last_opened_at_millis: Option<u64>,
    pub last_saved_at_millis: Option<u64>,
}

impl CatalogEntry {
    #[allow(dead_code)]
    fn recent_millis(&self) -> u64 {
        self.last_opened_at_millis
            .into_iter()
            .chain(self.last_saved_at_millis)
            .max()
            .unwrap_or(self.created_at_millis)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPathIdentity {
    pub exact_path: PathBuf,
    pub canonical_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CatalogFile {
    version: u32,
    sessions: Vec<CatalogEntry>,
}

impl Default for CatalogFile {
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            sessions: Vec::new(),
        }
    }
}

/// Path to the session catalog under the configured XDG data root.
pub fn catalog_path() -> PathBuf {
    catalog_dir().join("sessions.json")
}

/// Compute the exact/canonical identity used for dedupe without creating files.
pub fn session_path_identity(path: &Path) -> CatalogPathIdentity {
    let exact_path = normalize_exact_path(path);
    let canonical_path = existing_or_parent_canonical_path(&exact_path);
    CatalogPathIdentity {
        exact_path,
        canonical_path,
    }
}

/// Upsert a session in the catalog and mark it opened or saved.
pub fn upsert_session_event(path: &Path, event: CatalogEvent) -> Result<CatalogEntry> {
    with_catalog_write(|catalog| catalog.upsert(path, event))
}

/// Return recent sessions sorted newest first.
#[allow(dead_code)]
pub fn recent_sessions() -> Result<Vec<CatalogEntry>> {
    let path = catalog_path();
    let mut catalog = load_catalog_from_path(&path)?;
    catalog.sessions.sort_by(|a, b| {
        b.recent_millis()
            .cmp(&a.recent_millis())
            .then_with(|| a.display_name.cmp(&b.display_name))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(catalog.sessions)
}

/// Remove a catalog entry by opaque ID. Session files and sidecars are untouched.
#[allow(dead_code)]
pub fn forget_session_by_id(id: &str) -> Result<bool> {
    with_catalog_write(|catalog| {
        let before = catalog.sessions.len();
        catalog.sessions.retain(|entry| entry.id != id);
        Ok(before != catalog.sessions.len())
    })
}

/// Remove catalog entries for a path. Session files and sidecars are untouched.
#[allow(dead_code)]
pub fn forget_session_by_path(path: &Path) -> Result<bool> {
    let identity = session_path_identity(path);
    with_catalog_write(|catalog| {
        let before = catalog.sessions.len();
        catalog
            .sessions
            .retain(|entry| !entry_matches_identity(entry, &identity));
        Ok(before != catalog.sessions.len())
    })
}

pub(crate) fn record_named_session_opened(options: &SessionOptions) {
    if !options.is_named_file() {
        return;
    }
    let path = options.session_file_path();

    #[cfg(test)]
    if !test_catalog_hooks_enabled_for_path(&path) {
        return;
    }

    if let Err(err) = upsert_session_event(&path, CatalogEvent::Opened) {
        warn!(
            "Failed to update named session catalog after opening {}: {}",
            path.display(),
            err,
        );
    }
}

pub(crate) fn record_named_session_saved(options: &SessionOptions) {
    if !options.is_named_file() {
        return;
    }
    let path = options.session_file_path();

    #[cfg(test)]
    if !test_catalog_hooks_enabled_for_path(&path) {
        return;
    }

    if path.is_file()
        && let Err(err) = upsert_session_event(&path, CatalogEvent::Saved)
    {
        warn!(
            "Failed to update named session catalog after saving {}: {}",
            path.display(),
            err,
        );
    }
}

#[cfg(test)]
fn test_catalog_hooks_enabled_for_path(path: &Path) -> bool {
    let Some(raw) = std::env::var_os("WAYSCRIBER_ENABLE_CATALOG_HOOKS_IN_TESTS") else {
        return false;
    };
    if raw.is_empty() || raw == std::ffi::OsStr::new("1") {
        return true;
    }
    normalize_exact_path(path).starts_with(normalize_exact_path(Path::new(&raw)))
}

impl CatalogFile {
    fn upsert(&mut self, path: &Path, event: CatalogEvent) -> Result<CatalogEntry> {
        let identity = session_path_identity(path);
        let now = now_epoch_millis();

        if let Some(index) = self.entry_index(&identity) {
            let entry = &mut self.sessions[index];
            entry.path = path_to_string(&identity.exact_path)?;
            entry.canonical_path = optional_path_to_string(identity.canonical_path.as_deref())?;
            entry.display_name = display_name_for_path(&identity.exact_path);
            apply_event(entry, event, now);
            return Ok(entry.clone());
        }

        let mut entry = CatalogEntry {
            id: generated_catalog_id(now),
            display_name: display_name_for_path(&identity.exact_path),
            path: path_to_string(&identity.exact_path)?,
            canonical_path: optional_path_to_string(identity.canonical_path.as_deref())?,
            created_at_millis: now,
            last_opened_at_millis: None,
            last_saved_at_millis: None,
        };
        apply_event(&mut entry, event, now);
        self.sessions.push(entry.clone());
        Ok(entry)
    }

    fn entry_index(&self, identity: &CatalogPathIdentity) -> Option<usize> {
        self.sessions
            .iter()
            .position(|entry| entry_matches_identity(entry, identity))
    }
}

fn with_catalog_write<T>(update: impl FnOnce(&mut CatalogFile) -> Result<T>) -> Result<T> {
    let path = catalog_path();
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("session catalog path has no parent: {}", path.display()))?;
    fs::create_dir_all(dir).with_context(|| {
        format!(
            "failed to create session catalog directory {}",
            dir.display()
        )
    })?;

    let lock_path = catalog_lock_path(&path);
    let lock_file = open_runtime_lock_file(&lock_path, true).with_context(|| {
        format!(
            "failed to open session catalog lock {}",
            lock_path.display()
        )
    })?;
    lock_exclusive(&lock_file)
        .with_context(|| format!("failed to lock session catalog {}", lock_path.display()))?;

    let result = (|| {
        let mut catalog = load_catalog_from_path(&path)?;
        let value = update(&mut catalog)?;
        save_catalog_atomic(&path, &catalog)?;
        Ok(value)
    })();

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock session catalog {}: {}",
            lock_path.display(),
            err
        );
    }

    result
}

fn load_catalog_from_path(path: &Path) -> Result<CatalogFile> {
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(CatalogFile::default()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to read session catalog {}", path.display()));
        }
    };
    if raw.iter().all(u8::is_ascii_whitespace) {
        return Ok(CatalogFile::default());
    }
    let catalog: CatalogFile = serde_json::from_slice(&raw)
        .with_context(|| format!("failed to parse session catalog {}", path.display()))?;
    if catalog.version != CATALOG_VERSION {
        return Err(anyhow!(
            "unsupported session catalog version {} in {}",
            catalog.version,
            path.display()
        ));
    }
    Ok(catalog)
}

fn save_catalog_atomic(path: &Path, catalog: &CatalogFile) -> Result<()> {
    let tmp_path = catalog_temp_path(path)?;
    save_catalog_atomic_with_temp_path(path, &tmp_path, catalog)
}

fn save_catalog_atomic_with_temp_path(
    path: &Path,
    tmp_path: &Path,
    catalog: &CatalogFile,
) -> Result<()> {
    let payload =
        serde_json::to_vec_pretty(catalog).context("failed to serialize session catalog")?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary session catalog {}",
                    tmp_path.display()
                )
            })?;
        file.write_all(&payload)
            .context("failed to write session catalog")?;
        file.sync_all().context("failed to sync session catalog")?;
        drop(file);
        fs::rename(tmp_path, path).with_context(|| {
            format!(
                "failed to move temporary session catalog {} -> {}",
                tmp_path.display(),
                path.display()
            )
        })?;
        if let Err(err) = sync_catalog_parent(path) {
            warn!(
                "Session catalog {} was updated, but syncing its parent directory failed: {}",
                path.display(),
                err
            );
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp_path);
    }
    result
}

fn catalog_dir() -> PathBuf {
    crate::paths::data_dir()
        .unwrap_or_else(|| crate::paths::home_dir().unwrap_or_else(std::env::temp_dir))
        .join("wayscriber")
}

fn catalog_lock_path(path: &Path) -> PathBuf {
    let mut raw = std::ffi::OsString::from(path.as_os_str());
    raw.push(".lock");
    PathBuf::from(raw)
}

fn catalog_temp_path(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("session catalog path has no parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            anyhow!(
                "session catalog path must be valid UTF-8: {}",
                path.display()
            )
        })?;
    let id = NEXT_CATALOG_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".{file_name}.{}-{id}.tmp", std::process::id())))
}

#[cfg(unix)]
fn sync_catalog_parent(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let dir = File::open(parent)
        .with_context(|| format!("failed to open catalog directory {}", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to sync catalog directory {}", parent.display()))
}

#[cfg(not(unix))]
fn sync_catalog_parent(_path: &Path) -> Result<()> {
    Ok(())
}

fn apply_event(entry: &mut CatalogEntry, event: CatalogEvent, now: u64) {
    match event {
        CatalogEvent::Opened => entry.last_opened_at_millis = Some(now),
        CatalogEvent::Saved => entry.last_saved_at_millis = Some(now),
    }
}

fn entry_matches_identity(entry: &CatalogEntry, identity: &CatalogPathIdentity) -> bool {
    if path_string_matches(&entry.path, &identity.exact_path) {
        return true;
    }
    match (
        entry.canonical_path.as_deref(),
        identity.canonical_path.as_deref(),
    ) {
        (Some(existing), Some(candidate)) => path_string_matches(existing, candidate),
        _ => false,
    }
}

fn path_string_matches(existing: &str, candidate: &Path) -> bool {
    Path::new(existing) == candidate
}

fn existing_or_parent_canonical_path(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let file_name = path.file_name()?;
    parent
        .canonicalize()
        .ok()
        .map(|parent| parent.join(file_name))
}

fn normalize_exact_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => normalized.push(".."),
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn path_to_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        anyhow!(
            "session catalog path must be valid UTF-8: {}",
            path.display()
        )
    })
}

fn optional_path_to_string(path: Option<&Path>) -> Result<Option<String>> {
    path.map(path_to_string).transpose()
}

fn display_name_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("session")
        .to_string()
}

fn generated_catalog_id(now: u64) -> String {
    let counter = NEXT_CATALOG_ID.fetch_add(1, Ordering::Relaxed);
    format!("s-{:x}-{:x}-{counter:x}", std::process::id(), now)
}

fn now_epoch_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
