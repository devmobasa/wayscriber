use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use wayscriber::session::catalog::CatalogEntry;

const PATH_LABEL_MAX_CHARS: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCatalogState {
    pub items: Vec<SessionCatalogItem>,
    pub rename_inputs: HashMap<String, String>,
    pub duplicate_inputs: HashMap<String, String>,
    pub move_inputs: HashMap<String, String>,
    pub is_loading: bool,
    pub busy: bool,
    pub pending_clear_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCatalogActionResult {
    pub message: String,
    pub items: Vec<SessionCatalogItem>,
    pub warning: bool,
}

impl SessionCatalogActionResult {
    pub fn success(message: impl Into<String>, items: Vec<SessionCatalogItem>) -> Self {
        Self {
            message: message.into(),
            items,
            warning: false,
        }
    }

    pub fn warning(message: impl Into<String>, items: Vec<SessionCatalogItem>) -> Self {
        Self {
            message: message.into(),
            items,
            warning: true,
        }
    }
}

impl SessionCatalogState {
    pub fn loading() -> Self {
        Self {
            items: Vec::new(),
            rename_inputs: HashMap::new(),
            duplicate_inputs: HashMap::new(),
            move_inputs: HashMap::new(),
            is_loading: true,
            busy: false,
            pending_clear_id: None,
        }
    }

    pub fn replace_items(&mut self, items: Vec<SessionCatalogItem>) {
        self.rename_inputs = items
            .iter()
            .map(|item| (item.id.clone(), item.display_name.clone()))
            .collect();
        self.duplicate_inputs = items
            .iter()
            .map(|item| {
                (
                    item.id.clone(),
                    default_duplicate_target_path(&item.path)
                        .display()
                        .to_string(),
                )
            })
            .collect();
        self.move_inputs = items
            .iter()
            .map(|item| {
                (
                    item.id.clone(),
                    default_move_target_path(&item.path).display().to_string(),
                )
            })
            .collect();
        self.items = items;
        self.is_loading = false;
        self.busy = false;
        self.pending_clear_id = None;
    }

    pub fn rename_value(&self, id: &str, fallback: &str) -> String {
        self.rename_inputs
            .get(id)
            .cloned()
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn duplicate_value(&self, id: &str, source_path: &Path) -> String {
        self.duplicate_inputs.get(id).cloned().unwrap_or_else(|| {
            default_duplicate_target_path(source_path)
                .display()
                .to_string()
        })
    }

    pub fn move_value(&self, id: &str, source_path: &Path) -> String {
        self.move_inputs
            .get(id)
            .cloned()
            .unwrap_or_else(|| default_move_target_path(source_path).display().to_string())
    }

    pub fn item(&self, id: &str) -> Option<&SessionCatalogItem> {
        self.items.iter().find(|item| item.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCatalogItem {
    pub id: String,
    pub display_name: String,
    pub path: PathBuf,
    pub path_label: String,
    pub canonical_path_label: Option<String>,
    pub created_label: String,
    pub last_opened_label: String,
    pub last_saved_label: String,
    pub artifacts: SessionArtifactSummary,
}

impl SessionCatalogItem {
    pub fn from_entry(entry: CatalogEntry) -> Result<Self, String> {
        let path = PathBuf::from(&entry.path);
        let canonical_path_label = entry
            .canonical_path
            .as_ref()
            .map(|path| truncate_middle(path, PATH_LABEL_MAX_CHARS));
        let artifacts = SessionArtifactSummary::from_primary_path(&path)?;
        Ok(Self {
            id: entry.id,
            display_name: entry.display_name,
            path_label: truncate_path(&path),
            path,
            canonical_path_label,
            created_label: format_catalog_time(entry.created_at_millis),
            last_opened_label: entry
                .last_opened_at_millis
                .map(format_catalog_time)
                .unwrap_or_else(|| "Never".to_string()),
            last_saved_label: entry
                .last_saved_at_millis
                .map(format_catalog_time)
                .unwrap_or_else(|| "Never".to_string()),
            artifacts,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionArtifactSummary {
    pub primary_exists: bool,
    pub backup_exists: bool,
    pub recovery_exists: bool,
    pub clear_marker_exists: bool,
    pub lock_exists: bool,
    pub non_lock_size_bytes: u64,
}

impl SessionArtifactSummary {
    pub fn from_primary_path(path: &Path) -> Result<Self, String> {
        let artifacts = wayscriber::session::named_session_artifact_paths(path);
        let non_lock_paths = wayscriber::session::named_session_non_lock_artifact_paths(path)
            .map_err(|err| err.to_string())?;
        let recovery_name = artifacts
            .recovery
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);

        let mut summary = Self {
            primary_exists: artifact_exists(&artifacts.primary)?,
            backup_exists: artifact_exists(&artifacts.backup)?
                || artifact_exists(&artifacts.backup_recovery_marker)?,
            recovery_exists: false,
            clear_marker_exists: artifact_exists(&artifacts.clear_marker)?,
            lock_exists: artifact_exists(&artifacts.lock)?,
            non_lock_size_bytes: 0,
        };

        for path in non_lock_paths {
            let Some(metadata) = artifact_metadata(&path)? else {
                continue;
            };
            summary.non_lock_size_bytes =
                summary.non_lock_size_bytes.saturating_add(metadata.len());
            if recovery_name.as_deref().is_some_and(|name| {
                path.file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value == name || value.starts_with(&format!("{name}.")))
            }) {
                summary.recovery_exists = true;
            }
        }

        Ok(summary)
    }

    pub fn status_label(&self) -> String {
        let mut parts = Vec::new();
        if self.primary_exists {
            parts.push("primary");
        }
        if self.backup_exists {
            parts.push("backup");
        }
        if self.recovery_exists {
            parts.push("recovery");
        }
        if self.clear_marker_exists {
            parts.push("cleared");
        }
        if self.lock_exists {
            parts.push("lock");
        }
        if parts.is_empty() {
            "No saved artifacts".to_string()
        } else {
            format!(
                "{} · {}",
                parts.join(", "),
                format_byte_count(self.non_lock_size_bytes)
            )
        }
    }
}

fn artifact_exists(path: &Path) -> Result<bool, String> {
    Ok(artifact_metadata(path)?.is_some())
}

fn artifact_metadata(path: &Path) -> Result<Option<std::fs::Metadata>, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("failed to inspect {}: {err}", path.display())),
    }
}

pub fn format_byte_count(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;

    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / KIB)
    } else {
        format!("{:.1} MiB", bytes as f64 / MIB)
    }
}

fn format_catalog_time(epoch_millis: u64) -> String {
    let time = UNIX_EPOCH + Duration::from_millis(epoch_millis);
    wayscriber::time_utils::format_system_time(time, "%Y-%m-%d %H:%M")
        .unwrap_or_else(|| "Unknown".to_string())
}

fn truncate_path(path: &Path) -> String {
    truncate_middle(&path.display().to_string(), PATH_LABEL_MAX_CHARS)
}

fn default_duplicate_target_path(path: &Path) -> PathBuf {
    default_sibling_target_path(path, "copy")
}

fn default_move_target_path(path: &Path) -> PathBuf {
    default_sibling_target_path(path, "moved")
}

fn default_sibling_target_path(path: &Path, suffix_word: &str) -> PathBuf {
    const SESSION_SUFFIX: &str = ".wayscriber-session";

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("session.wayscriber-session");
    let target_name = file_name
        .strip_suffix(SESSION_SUFFIX)
        .map(|stem| format!("{stem} {suffix_word}{SESSION_SUFFIX}"))
        .unwrap_or_else(|| format!("{file_name} {suffix_word}"));

    match path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(parent) => parent.join(target_name),
        None => PathBuf::from(target_name),
    }
}

fn truncate_middle(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }

    let keep = max_chars - 3;
    let start_count = keep / 2;
    let end_count = keep - start_count;
    let start = value.chars().take(start_count).collect::<String>();
    let end = value
        .chars()
        .rev()
        .take(end_count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{start}...{end}")
}

#[cfg(test)]
mod tests;
