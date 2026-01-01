use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use super::types::ClearOutcome;
use crate::session::options::SessionOptions;

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
