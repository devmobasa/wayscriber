use super::options::SessionOptions;
use super::snapshot;
use anyhow::{Context, Result};
use log::warn;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Result of clearing on-disk session data.
#[derive(Debug, Clone, Copy)]
pub struct ClearOutcome {
    pub removed_session: bool,
    pub removed_backup: bool,
    pub removed_lock: bool,
}

/// Summary information about the current session file(s).
#[derive(Debug, Clone)]
pub struct SessionInspection {
    pub session_path: PathBuf,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub modified: Option<SystemTime>,
    pub backup_path: PathBuf,
    pub backup_exists: bool,
    pub backup_size_bytes: Option<u64>,
    pub active_identity: Option<String>,
    pub per_output: bool,
    pub persist_transparent: bool,
    pub persist_whiteboard: bool,
    pub persist_blackboard: bool,
    pub restore_tool_state: bool,
    pub frame_counts: Option<FrameCounts>,
    pub tool_state_present: bool,
    pub compressed: bool,
}

/// Frame counts for each board stored in the session.
#[derive(Debug, Clone, Copy)]
pub struct FrameCounts {
    pub transparent: usize,
    pub whiteboard: usize,
    pub blackboard: usize,
}

/// Remove persisted session files (session, backup, and lock).
pub fn clear_session(options: &SessionOptions) -> Result<ClearOutcome> {
    let session_path = options.session_file_path();
    let backup_path = options.backup_file_path();
    let lock_path = options.lock_file_path();

    let mut removed_session = remove_file_if_exists(&session_path)?;
    let mut removed_backup = remove_file_if_exists(&backup_path)?;
    let mut removed_lock = remove_file_if_exists(&lock_path)?;

    if options.per_output && options.output_identity().is_none() {
        let prefix = options.file_prefix();
        let base_dir = &options.base_dir;

        if !removed_session {
            removed_session = remove_matching_files(base_dir, &prefix, ".json")? || removed_session;
        }

        if !removed_backup {
            removed_backup =
                remove_matching_files(base_dir, &prefix, ".json.bak")? || removed_backup;
        }

        if !removed_lock {
            removed_lock = remove_matching_files(base_dir, &prefix, ".lock")? || removed_lock;
        }
    }

    Ok(ClearOutcome {
        removed_session,
        removed_backup,
        removed_lock,
    })
}

/// Inspect the current session file for CLI reporting.
pub fn inspect_session(options: &SessionOptions) -> Result<SessionInspection> {
    let prefix = options.file_prefix();
    let mut session_path = options.session_file_path();
    let mut session_identity = options.output_identity().map(|s| s.to_string());
    let mut metadata = fs::metadata(&session_path).ok();

    if metadata.is_none() && options.per_output && options.output_identity().is_none() {
        if let Some((path, identity)) = find_existing_variant(&options.base_dir, &prefix, ".json") {
            metadata = fs::metadata(&path).ok();
            session_path = path;
            session_identity = identity;
        }
    }

    let exists = metadata.is_some();
    let size_bytes = metadata.as_ref().map(|m| m.len());
    let modified = metadata.as_ref().and_then(|m| m.modified().ok());

    let mut backup_path = options.backup_file_path();
    let mut backup_meta = fs::metadata(&backup_path).ok();
    if backup_meta.is_none() && options.per_output && options.output_identity().is_none() {
        if let Some((path, _)) = find_existing_variant(&options.base_dir, &prefix, ".json.bak") {
            backup_meta = fs::metadata(&path).ok();
            backup_path = path;
        }
    }

    let backup_exists = backup_meta.is_some();
    let backup_size = backup_meta.as_ref().map(|m| m.len());

    let mut frame_counts = None;
    let mut tool_state_present = false;
    let mut compressed = false;

    if exists {
        let lock_path = session_path.with_extension("lock");
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
        lock_file
            .lock_shared()
            .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;

        let loaded = snapshot::load_snapshot_inner(&session_path, options);

        lock_file.unlock().unwrap_or_else(|err| {
            warn!(
                "failed to unlock session file {}: {}",
                lock_path.display(),
                err
            )
        });

        if let Some(loaded) = loaded? {
            frame_counts = Some(FrameCounts {
                transparent: loaded
                    .snapshot
                    .transparent
                    .as_ref()
                    .map_or(0, |f| f.shapes.len()),
                whiteboard: loaded
                    .snapshot
                    .whiteboard
                    .as_ref()
                    .map_or(0, |f| f.shapes.len()),
                blackboard: loaded
                    .snapshot
                    .blackboard
                    .as_ref()
                    .map_or(0, |f| f.shapes.len()),
            });
            tool_state_present = loaded.snapshot.tool_state.is_some();
            compressed = loaded.compressed;
        }
    }

    Ok(SessionInspection {
        session_path,
        exists,
        size_bytes,
        modified,
        backup_path,
        backup_exists,
        backup_size_bytes: backup_size,
        active_identity: session_identity,
        per_output: options.per_output,
        persist_transparent: options.persist_transparent,
        persist_whiteboard: options.persist_whiteboard,
        persist_blackboard: options.persist_blackboard,
        restore_tool_state: options.restore_tool_state,
        frame_counts,
        tool_state_present,
        compressed,
    })
}

fn remove_file_if_exists(path: &Path) -> Result<bool> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn remove_matching_files(dir: &Path, prefix: &str, suffix: &str) -> Result<bool> {
    let mut removed = false;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
            {
                if name.starts_with(prefix) && name.ends_with(suffix) {
                    fs::remove_file(&path)
                        .with_context(|| format!("failed to remove {}", path.display()))?;
                    removed = true;
                }
            }
        }
    }
    Ok(removed)
}

fn find_existing_variant(
    dir: &Path,
    prefix: &str,
    suffix: &str,
) -> Option<(PathBuf, Option<String>)> {
    let entries = fs::read_dir(dir).ok()?;
    let mut matches: Vec<(PathBuf, Option<String>)> = Vec::new();

    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
        {
            if name.starts_with(prefix) && name.ends_with(suffix) {
                matches.push((path, extract_identity(&name, prefix, suffix)));
            }
        }
    }

    matches.sort_by(|a, b| {
        let a_name = a.0.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        let b_name = b.0.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        a_name.cmp(b_name)
    });

    matches.into_iter().next()
}

fn extract_identity(name: &str, prefix: &str, suffix: &str) -> Option<String> {
    if !name.starts_with(prefix) || !name.ends_with(suffix) {
        return None;
    }

    let start = prefix.len();
    let end = name.len() - suffix.len();
    if start >= end {
        return None;
    }

    let trimmed = name[start..end].trim_start_matches('-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
