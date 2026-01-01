use super::compression::{compress_bytes, temp_path};
use super::types::{CURRENT_VERSION, SessionFile, SessionSnapshot};
use crate::input::board_mode::BoardMode;
use crate::session::lock::{lock_exclusive, unlock};
use crate::session::options::{CompressionMode, SessionOptions};
use crate::time_utils::now_rfc3339;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::fs::{self, OpenOptions};
use std::io::Write;

/// Persist the provided snapshot to disk according to the configured options.
pub fn save_snapshot(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
    if !options.any_enabled() && snapshot.tool_state.is_none() {
        debug!("Session persistence disabled for all boards; skipping save");
        return Ok(());
    }

    fs::create_dir_all(&options.base_dir).with_context(|| {
        format!(
            "failed to create session directory {}",
            options.base_dir.display()
        )
    })?;

    let lock_path = options.lock_file_path();
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_exclusive(&lock_file)
        .with_context(|| format!("failed to lock session file {}", lock_path.display()))?;

    let result = save_snapshot_inner(snapshot, options);

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        );
    }

    result
}

fn save_snapshot_inner(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
    let session_path = options.session_file_path();
    let backup_path = options.backup_file_path();

    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        if session_path.exists() {
            debug!(
                "Removing session file {} because snapshot is empty",
                session_path.display()
            );
            fs::remove_file(&session_path).with_context(|| {
                format!(
                    "failed to remove empty session file {}",
                    session_path.display()
                )
            })?;
        }
        return Ok(());
    }

    let transparent = snapshot.transparent.clone();
    let whiteboard = snapshot.whiteboard.clone();
    let blackboard = snapshot.blackboard.clone();

    let file_payload = SessionFile {
        version: CURRENT_VERSION,
        last_modified: now_rfc3339(),
        active_mode: board_mode_to_str(snapshot.active_mode).to_string(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        transparent_pages: transparent.as_ref().map(|pages| pages.pages.clone()),
        whiteboard_pages: whiteboard.as_ref().map(|pages| pages.pages.clone()),
        blackboard_pages: blackboard.as_ref().map(|pages| pages.pages.clone()),
        transparent_active_page: transparent.as_ref().map(|pages| pages.active),
        whiteboard_active_page: whiteboard.as_ref().map(|pages| pages.active),
        blackboard_active_page: blackboard.as_ref().map(|pages| pages.active),
        tool_state: snapshot.tool_state.clone(),
    };

    let mut json_bytes =
        serde_json::to_vec_pretty(&file_payload).context("failed to serialise session payload")?;

    if json_bytes.len() as u64 > options.max_file_size_bytes {
        warn!(
            "Session data size {} bytes exceeds the configured limit of {} bytes; skipping save",
            json_bytes.len(),
            options.max_file_size_bytes
        );
        return Ok(());
    }

    let should_compress = match options.compression {
        CompressionMode::Off => false,
        CompressionMode::On => true,
        CompressionMode::Auto => (json_bytes.len() as u64) >= options.auto_compress_threshold_bytes,
    };

    if should_compress {
        json_bytes = compress_bytes(&json_bytes)?;
    }

    let tmp_path = temp_path(&session_path)?;
    {
        let mut tmp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
            .with_context(|| {
                format!(
                    "failed to open temporary session file {}",
                    tmp_path.display()
                )
            })?;
        tmp_file
            .write_all(&json_bytes)
            .context("failed to write session payload")?;
        tmp_file
            .sync_all()
            .context("failed to sync temporary session file")?;
    }

    if session_path.exists() {
        if options.backup_retention > 0 {
            if backup_path.exists() {
                fs::remove_file(&backup_path).ok();
            }
            fs::rename(&session_path, &backup_path).with_context(|| {
                format!(
                    "failed to rotate previous session file {} -> {}",
                    session_path.display(),
                    backup_path.display()
                )
            })?;
        } else {
            fs::remove_file(&session_path).ok();
        }
    }

    fs::rename(&tmp_path, &session_path).with_context(|| {
        format!(
            "failed to move temporary session file {} -> {}",
            tmp_path.display(),
            session_path.display()
        )
    })?;

    info!(
        "Session saved to {} ({} bytes, compression={})",
        session_path.display(),
        json_bytes.len(),
        should_compress
    );

    Ok(())
}

fn board_mode_to_str(mode: BoardMode) -> &'static str {
    match mode {
        BoardMode::Transparent => "transparent",
        BoardMode::Whiteboard => "whiteboard",
        BoardMode::Blackboard => "blackboard",
    }
}
