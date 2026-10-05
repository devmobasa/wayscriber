use crate::session::artifacts::{parse_corrupt_copy_name, remove_corrupt_copies};
use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
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
    removed_session = remove_corrupt_copies(&session_path)? || removed_session;
    removed_backup = remove_corrupt_copies(&backup_path)? || removed_backup;
    removed_backup = remove_file_if_exists(&backup_recovery_marker_path)? || removed_backup;
    let mut removed_recovery = remove_recovery_files(&recovery_path)?;
    let mut removed_lock = remove_file_if_exists(&lock_path)?;

    if options.per_output && options.output_identity().is_none() {
        let prefix = options.file_prefix();
        let base_dir = &options.base_dir;
        // Every output's files carry this session's suffixes after the prefix.
        let session_suffix = suffix_after_prefix(&session_path, &prefix)?;
        let backup_suffix = suffix_after_prefix(&backup_path, &prefix)?;

        let removed_matching_sessions = remove_matching_files(base_dir, &prefix, &session_suffix)?;
        let removed_matching_clear_markers =
            remove_matching_files(base_dir, &prefix, ".json.cleared")?;
        removed_session =
            removed_matching_sessions || removed_matching_clear_markers || removed_session;

        removed_backup =
            remove_matching_files(base_dir, &prefix, &backup_suffix)? || removed_backup;
        removed_backup =
            remove_matching_files(base_dir, &prefix, ".json.bak.recoverable")? || removed_backup;

        removed_recovery = remove_matching_recovery_files(base_dir, &prefix)? || removed_recovery;

        removed_lock = remove_matching_files(base_dir, &prefix, ".lock")? || removed_lock;
        let (copied_session, copied_backup) =
            remove_matching_corrupt_copies(base_dir, &prefix, &session_suffix, &backup_suffix)?;
        removed_session = copied_session || removed_session;
        removed_backup = copied_backup || removed_backup;
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
    let Some(recovery_name) = recovery_path.file_name() else {
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
            let Some(name) = path.file_name() else {
                continue;
            };
            if crate::session::artifacts::is_recovery_variant(name, recovery_name) {
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

fn suffix_after_prefix(path: &Path, prefix: &str) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(prefix))
        .map(str::to_owned)
        .with_context(|| format!("{} is not named after {prefix}", path.display()))
}

/// Removes the `.corrupt-N` copies of every output's session and backup files,
/// reporting whether any of each were removed.
fn remove_matching_corrupt_copies(
    dir: &Path,
    prefix: &str,
    session_suffix: &str,
    backup_suffix: &str,
) -> Result<(bool, bool)> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((false, false)),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to list {}", dir.display()));
        }
    };

    let (mut removed_session, mut removed_backup) = (false, false);
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(artifact) =
            parse_corrupt_copy_name(&name).and_then(|(artifact, _)| artifact.to_str())
        else {
            continue;
        };
        let backup = artifact.ends_with(backup_suffix);
        if !name_matches_session_prefix(artifact, prefix)
            || !(backup || artifact.ends_with(session_suffix))
            || !entry.file_type()?.is_file()
        {
            continue;
        }

        let path = entry.path();
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
        if backup {
            removed_backup = true;
        } else {
            removed_session = true;
        }
    }

    Ok((removed_session, removed_backup))
}

fn name_matches_session_prefix(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('.') || rest.starts_with('-'))
}
