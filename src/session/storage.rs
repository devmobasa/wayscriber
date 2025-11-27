use super::options::SessionOptions;
use super::snapshot;
use crate::draw::Frame;
use crate::session::lock::{lock_shared, unlock};
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
    pub persist_history: bool,
    pub restore_tool_state: bool,
    pub history_limit: Option<usize>,
    pub frame_counts: Option<FrameCounts>,
    pub history_counts: Option<HistoryCounts>,
    pub history_present: bool,
    pub tool_state_present: bool,
    pub compressed: bool,
    pub file_version: Option<u32>,
}

/// Frame counts for each board stored in the session.
#[derive(Debug, Clone, Copy)]
pub struct FrameCounts {
    pub transparent: usize,
    pub whiteboard: usize,
    pub blackboard: usize,
}

/// Undo/redo counts for each board stored in the session.
#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryCounts {
    pub transparent: HistoryDepth,
    pub whiteboard: HistoryDepth,
    pub blackboard: HistoryDepth,
}

impl HistoryCounts {
    fn has_history(&self) -> bool {
        self.transparent.has_history()
            || self.whiteboard.has_history()
            || self.blackboard.has_history()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryDepth {
    pub undo: usize,
    pub redo: usize,
}

impl HistoryDepth {
    fn has_history(&self) -> bool {
        self.undo > 0 || self.redo > 0
    }
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

    if metadata.is_none()
        && options.per_output
        && options.output_identity().is_none()
        && let Some((path, identity)) = find_existing_variant(&options.base_dir, &prefix, ".json")
    {
        metadata = fs::metadata(&path).ok();
        session_path = path;
        session_identity = identity;
    }

    let exists = metadata.is_some();
    let size_bytes = metadata.as_ref().map(|m| m.len());
    let modified = metadata.as_ref().and_then(|m| m.modified().ok());

    let mut backup_path = options.backup_file_path();
    let mut backup_meta = fs::metadata(&backup_path).ok();
    if backup_meta.is_none()
        && options.per_output
        && options.output_identity().is_none()
        && let Some((path, _)) = find_existing_variant(&options.base_dir, &prefix, ".json.bak")
    {
        backup_meta = fs::metadata(&path).ok();
        backup_path = path;
    }

    let backup_exists = backup_meta.is_some();
    let backup_size = backup_meta.as_ref().map(|m| m.len());

    let mut frame_counts = None;
    let mut tool_state_present = false;
    let mut compressed = false;
    let mut history_counts = None;
    let mut history_present = false;
    let mut file_version = None;

    if exists {
        let lock_path = session_path.with_extension("lock");
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&lock_path)
            .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
        lock_shared(&lock_file)
            .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;

        let loaded = snapshot::load_snapshot_inner(&session_path, options);

        if let Err(err) = unlock(&lock_file) {
            warn!(
                "failed to unlock session file {}: {}",
                lock_path.display(),
                err
            );
        }

        if let Some(loaded) = loaded? {
            let snapshot = loaded.snapshot;
            frame_counts = Some(FrameCounts {
                transparent: snapshot.transparent.as_ref().map_or(0, |f| f.shapes.len()),
                whiteboard: snapshot.whiteboard.as_ref().map_or(0, |f| f.shapes.len()),
                blackboard: snapshot.blackboard.as_ref().map_or(0, |f| f.shapes.len()),
            });
            let counts = HistoryCounts {
                transparent: history_depth_from_frame(snapshot.transparent.as_ref()),
                whiteboard: history_depth_from_frame(snapshot.whiteboard.as_ref()),
                blackboard: history_depth_from_frame(snapshot.blackboard.as_ref()),
            };
            history_present = counts.has_history();
            history_counts = Some(counts);
            tool_state_present = snapshot.tool_state.is_some();
            compressed = loaded.compressed;
            file_version = Some(loaded.version);
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
        persist_history: options.persist_history,
        restore_tool_state: options.restore_tool_state,
        history_limit: options.max_persisted_undo_depth,
        frame_counts,
        history_counts,
        history_present,
        tool_state_present,
        compressed,
        file_version,
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
                && name.starts_with(prefix)
                && name.ends_with(suffix)
            {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
                removed = true;
            }
        }
    }
    Ok(removed)
}

fn history_depth_from_frame(frame: Option<&Frame>) -> HistoryDepth {
    if let Some(frame) = frame {
        HistoryDepth {
            undo: frame.undo_stack_len(),
            redo: frame.redo_stack_len(),
        }
    } else {
        HistoryDepth::default()
    }
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
            && name.starts_with(prefix)
            && name.ends_with(suffix)
        {
            matches.push((path, extract_identity(&name, prefix, suffix)));
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
