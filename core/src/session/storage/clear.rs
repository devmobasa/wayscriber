use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::types::ClearOutcome;
use crate::session::options::SessionOptions;

/// Remove persisted session files (session, backup, and lock).
pub fn clear_session(options: &SessionOptions) -> Result<ClearOutcome> {
    let session_path = options.session_file_path();
    if options.is_named_file() {
        crate::session::validate_named_session_file_for_clear(&session_path)?;
    }
    let backup_path = options.backup_file_path();
    let backup_recovery_marker_path = options.backup_recovery_marker_file_path();
    let recovery_path = options.recovery_file_path();
    let clear_marker_path = options.clear_marker_file_path();
    let lock_path = options.lock_file_path();

    let removed_primary_session = remove_file_if_exists(&session_path)?;
    let removed_clear_marker = remove_file_if_exists(&clear_marker_path)?;
    let mut removed_session = removed_primary_session || removed_clear_marker;
    let mut removed_backup = remove_file_if_exists(&backup_path)?;
    removed_backup = remove_file_if_exists(&backup_recovery_marker_path)? || removed_backup;
    let mut removed_recovery = remove_recovery_files(&recovery_path)?;
    let mut removed_lock = remove_file_if_exists(&lock_path)?;

    if options.per_output && options.output_identity().is_none() {
        let prefix = options.file_prefix();
        let base_dir = &options.base_dir;

        let removed_matching_sessions = remove_matching_files(base_dir, &prefix, ".json")?;
        let removed_matching_clear_markers =
            remove_matching_files(base_dir, &prefix, ".json.cleared")?;
        removed_session =
            removed_matching_sessions || removed_matching_clear_markers || removed_session;

        removed_backup = remove_matching_files(base_dir, &prefix, ".json.bak")? || removed_backup;
        removed_backup =
            remove_matching_files(base_dir, &prefix, ".json.bak.recoverable")? || removed_backup;

        removed_recovery = remove_matching_recovery_files(base_dir, &prefix)? || removed_recovery;

        removed_lock = remove_matching_files(base_dir, &prefix, ".lock")? || removed_lock;
    }

    Ok(ClearOutcome {
        removed_session,
        removed_backup,
        removed_recovery,
        removed_lock,
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

fn remove_recovery_files(recovery_path: &Path) -> Result<bool> {
    let Some(recovery_name) = recovery_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
    else {
        return remove_file_if_exists(recovery_path);
    };
    let Some(parent) = recovery_path.parent() else {
        return remove_file_if_exists(recovery_path);
    };

    let mut removed = false;
    if let Ok(entries) = fs::read_dir(parent) {
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name == recovery_name || name.starts_with(&format!("{recovery_name}.")) {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
                removed = true;
            }
        }
    }
    Ok(removed)
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
                && name_matches_session_prefix(&name, prefix)
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

fn remove_matching_recovery_files(dir: &Path, prefix: &str) -> Result<bool> {
    let mut removed = false;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name_matches_session_prefix(name, prefix)
                && name.contains(".json.recovery")
            {
                fs::remove_file(&path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
                removed = true;
            }
        }
    }
    Ok(removed)
}

fn name_matches_session_prefix(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('-'))
}
