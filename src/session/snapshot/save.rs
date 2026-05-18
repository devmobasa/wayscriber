use super::compression::{compress_bytes, temp_path};
use super::types::{
    BoardFile, BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
};
use crate::session::lock::{lock_exclusive, unlock};
use crate::session::options::{CompressionMode, SessionOptions};
use crate::time_utils::now_rfc3339;
use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// Persist the provided snapshot to disk according to the configured options.
pub fn save_snapshot(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
    if !options.any_enabled() && !options.persist_history && snapshot.tool_state.is_none() {
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
    let last_modified = now_rfc3339();

    let Some(mut json_bytes) = payload_within_limit(snapshot, options, &last_modified)? else {
        remove_session_file(&session_path)?;
        return Ok(());
    };

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

fn payload_within_limit(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    last_modified: &str,
) -> Result<Option<Vec<u8>>> {
    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        return Ok(None);
    }

    let full_payload = serialize_payload(snapshot, last_modified)?;
    if full_payload.len() as u64 <= options.max_file_size_bytes {
        return Ok(Some(full_payload));
    }

    let full_size = full_payload.len();
    let history_depth = max_history_depth(snapshot);
    if history_depth > 0
        && let Some((depth, payload)) =
            largest_fitting_history_payload(snapshot, history_depth, options, last_modified)?
    {
        warn!(
            "Session data size {} bytes exceeds the configured limit of {} bytes; saving recent {} undo/redo history entries per stack ({} bytes)",
            full_size,
            options.max_file_size_bytes,
            depth,
            payload.len()
        );
        return Ok(Some(payload));
    }

    let visible_only = snapshot_without_history(snapshot);
    if visible_only.is_empty() && visible_only.tool_state.is_none() {
        warn!(
            "Session data size {} bytes exceeds the configured limit of {} bytes; dropping undo/redo history leaves no visible session data, clearing saved session",
            full_size, options.max_file_size_bytes
        );
        return Ok(None);
    }

    let visible_payload = serialize_payload(&visible_only, last_modified)?;
    if visible_payload.len() as u64 <= options.max_file_size_bytes {
        warn!(
            "Session data size {} bytes exceeds the configured limit of {} bytes; saving visible data without undo/redo history ({} bytes)",
            full_size,
            options.max_file_size_bytes,
            visible_payload.len()
        );
        return Ok(Some(visible_payload));
    }

    Err(anyhow!(
        "Session data size {} bytes exceeds the configured limit of {} bytes; skipping save",
        visible_payload.len(),
        options.max_file_size_bytes
    ))
}

fn largest_fitting_history_payload(
    snapshot: &SessionSnapshot,
    max_depth: usize,
    options: &SessionOptions,
    last_modified: &str,
) -> Result<Option<(usize, Vec<u8>)>> {
    let mut low = 1;
    let mut high = max_depth;
    let mut best = None;

    while low <= high {
        let depth = low + (high - low) / 2;
        let candidate = snapshot_with_history_depth(snapshot, depth);
        let payload = serialize_payload(&candidate, last_modified)?;

        if payload.len() as u64 <= options.max_file_size_bytes {
            best = Some((depth, payload));
            low = depth.saturating_add(1);
        } else if depth == 1 {
            break;
        } else {
            high = depth - 1;
        }
    }

    Ok(best)
}

fn serialize_payload(snapshot: &SessionSnapshot, last_modified: &str) -> Result<Vec<u8>> {
    let file_payload = SessionFile {
        version: CURRENT_VERSION,
        last_modified: last_modified.to_string(),
        active_board_id: Some(snapshot.active_board_id.clone()),
        active_mode: None,
        boards: snapshot
            .boards
            .iter()
            .map(|board| BoardFile {
                id: board.id.clone(),
                pages: board.pages.pages.clone(),
                active_page: board.pages.active,
            })
            .collect(),
        transparent: None,
        whiteboard: None,
        blackboard: None,
        transparent_pages: None,
        whiteboard_pages: None,
        blackboard_pages: None,
        transparent_active_page: None,
        whiteboard_active_page: None,
        blackboard_active_page: None,
        tool_state: snapshot.tool_state.clone(),
    };

    serde_json::to_vec_pretty(&file_payload).context("failed to serialise session payload")
}

fn max_history_depth(snapshot: &SessionSnapshot) -> usize {
    snapshot
        .boards
        .iter()
        .flat_map(|board| board.pages.pages.iter())
        .map(|page| page.undo_stack_len().max(page.redo_stack_len()))
        .max()
        .unwrap_or(0)
}

fn snapshot_with_history_depth(snapshot: &SessionSnapshot, depth: usize) -> SessionSnapshot {
    let mut candidate = snapshot.clone();
    for board in &mut candidate.boards {
        for page in &mut board.pages.pages {
            page.clamp_history_depth(depth);
        }
    }
    candidate
}

fn snapshot_without_history(snapshot: &SessionSnapshot) -> SessionSnapshot {
    let mut boards = Vec::with_capacity(snapshot.boards.len());
    for board in &snapshot.boards {
        let pages = BoardPagesSnapshot {
            pages: board
                .pages
                .pages
                .iter()
                .map(|page| page.clone_without_history())
                .collect(),
            active: board.pages.active,
        };
        if pages.has_persistable_data() {
            boards.push(BoardSnapshot {
                id: board.id.clone(),
                pages,
            });
        }
    }

    SessionSnapshot {
        active_board_id: snapshot.active_board_id.clone(),
        boards,
        tool_state: snapshot.tool_state.clone(),
    }
}

fn remove_session_file(session_path: &Path) -> Result<()> {
    if session_path.exists() {
        debug!(
            "Removing session file {} because snapshot is empty",
            session_path.display()
        );
        fs::remove_file(session_path).with_context(|| {
            format!(
                "failed to remove empty session file {}",
                session_path.display()
            )
        })?;
    }
    Ok(())
}
