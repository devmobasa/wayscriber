use super::compression::maybe_decompress;
use super::history::{
    apply_history_policies, enforce_shape_limits, max_history_depth, strip_history_fields,
};
use super::types::{
    BoardFile, BoardPagesSnapshot, BoardSnapshot, CURRENT_VERSION, SessionFile, SessionSnapshot,
};
use crate::draw::Frame;
use crate::draw::frame::MAX_COMPOUND_DEPTH;
use crate::session::lock::{lock_shared, unlock};
use crate::session::options::SessionOptions;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::Path;

pub struct LoadedSnapshot {
    pub snapshot: SessionSnapshot,
    pub compressed: bool,
    pub version: u32,
}

/// Attempt to load a previously saved session.
pub fn load_snapshot(options: &SessionOptions) -> Result<Option<SessionSnapshot>> {
    if !options.any_enabled() && !options.restore_tool_state {
        info!(
            "Session load skipped: persistence disabled (base_dir={}, file={})",
            options.base_dir.display(),
            options.session_file_path().display()
        );
        return Ok(None);
    }

    let session_path = options.session_file_path();
    if !session_path.exists() {
        info!(
            "Session file not found at {}; skipping load",
            session_path.display()
        );
        return Ok(None);
    }

    let metadata = fs::metadata(&session_path)
        .with_context(|| format!("failed to stat session file {}", session_path.display()))?;
    info!(
        "Session file present at {} ({} bytes, per_output={}, output_identity={:?})",
        session_path.display(),
        metadata.len(),
        options.per_output,
        options.output_identity()
    );
    if metadata.len() > options.max_file_size_bytes {
        warn!(
            "Session file {} is {} bytes which exceeds the configured limit ({} bytes); refusing to load",
            session_path.display(),
            metadata.len(),
            options.max_file_size_bytes
        );
        return Ok(None);
    }

    let lock_path = options.lock_file_path();
    let lock_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_shared(&lock_file)
        .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;

    let result = load_snapshot_inner(&session_path, options);

    if let Err(err) = unlock(&lock_file) {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        );
    }

    match result {
        Ok(Some(loaded)) => {
            let tool_state = loaded.snapshot.tool_state.is_some();
            info!(
                "Loaded session from {} (version {}, compressed={}, boards={}, active_board={}, tool_state={})",
                session_path.display(),
                loaded.version,
                loaded.compressed,
                loaded.snapshot.boards.len(),
                loaded.snapshot.active_board_id,
                tool_state
            );
            Ok(Some(loaded.snapshot))
        }
        Ok(None) => {
            info!(
                "Session file {} contained no usable data; continuing with defaults",
                session_path.display()
            );
            Ok(None)
        }
        Err(err) => {
            warn!(
                "Failed to load session {}; backing up and continuing with defaults: {}",
                session_path.display(),
                err
            );
            if let Err(backup_err) = backup_corrupt_session(&session_path, options) {
                warn!(
                    "Failed to back up corrupt session {}: {}",
                    session_path.display(),
                    backup_err
                );
            }
            Ok(None)
        }
    }
}

pub(crate) fn load_snapshot_inner(
    session_path: &Path,
    options: &SessionOptions,
) -> Result<Option<LoadedSnapshot>> {
    let mut file_bytes = Vec::new();
    {
        let mut file = File::open(session_path)
            .with_context(|| format!("failed to open session file {}", session_path.display()))?;
        file.read_to_end(&mut file_bytes)
            .context("failed to read session file")?;
    }

    let (decompressed, compressed) = maybe_decompress(file_bytes)?;

    let original_value: Value =
        serde_json::from_slice(&decompressed).context("failed to parse session json")?;

    let max_depth = max_history_depth(&original_value);
    let mut working_value = original_value.clone();
    if max_depth > MAX_COMPOUND_DEPTH {
        warn!(
            "Session history depth {} exceeds limit {}; dropping history",
            max_depth, MAX_COMPOUND_DEPTH
        );
        strip_history_fields(&mut working_value);
    }

    let session_file: SessionFile = match serde_json::from_value(working_value.clone()) {
        Ok(file) => file,
        Err(err) => {
            warn!(
                "Failed to deserialize session ({}); retrying without history",
                err
            );
            let mut stripped = original_value.clone();
            strip_history_fields(&mut stripped);
            serde_json::from_value(stripped)
                .context("failed to parse session after stripping history")?
        }
    };

    if session_file.version > CURRENT_VERSION {
        warn!(
            "Session file version {} is newer than supported version {}; skipping load",
            session_file.version, CURRENT_VERSION
        );
        return Ok(None);
    }

    let SessionFile {
        active_board_id,
        active_mode,
        boards,
        transparent,
        whiteboard,
        blackboard,
        transparent_pages,
        whiteboard_pages,
        blackboard_pages,
        transparent_active_page,
        whiteboard_active_page,
        blackboard_active_page,
        tool_state,
        ..
    } = session_file;

    let pages_from_file = |pages: Option<Vec<Frame>>,
                           active: Option<usize>,
                           legacy: Option<Frame>|
     -> Option<BoardPagesSnapshot> {
        if let Some(mut pages) = pages {
            if pages.is_empty() {
                pages.push(Frame::new());
            }
            let active = active.unwrap_or(0).min(pages.len() - 1);
            return Some(BoardPagesSnapshot { pages, active });
        }
        legacy.map(|frame| BoardPagesSnapshot {
            pages: vec![frame],
            active: 0,
        })
    };

    let mut snapshot = if !boards.is_empty() || active_board_id.is_some() {
        let mut board_snaps = Vec::new();
        for BoardFile {
            id,
            mut pages,
            active_page,
        } in boards
        {
            if pages.is_empty() {
                pages.push(Frame::new());
            }
            let active = active_page.min(pages.len().saturating_sub(1));
            board_snaps.push(BoardSnapshot {
                id,
                pages: BoardPagesSnapshot { pages, active },
            });
        }
        let fallback_id = board_snaps
            .first()
            .map(|board| board.id.clone())
            .unwrap_or_else(|| "transparent".to_string());
        let active_board_id = active_board_id.unwrap_or(fallback_id);
        SessionSnapshot {
            active_board_id,
            boards: board_snaps,
            tool_state,
        }
    } else {
        let mut board_snaps = Vec::new();
        if let Some(pages) =
            pages_from_file(transparent_pages, transparent_active_page, transparent)
        {
            board_snaps.push(BoardSnapshot {
                id: "transparent".to_string(),
                pages,
            });
        }
        if let Some(pages) = pages_from_file(whiteboard_pages, whiteboard_active_page, whiteboard) {
            board_snaps.push(BoardSnapshot {
                id: "whiteboard".to_string(),
                pages,
            });
        }
        if let Some(pages) = pages_from_file(blackboard_pages, blackboard_active_page, blackboard) {
            board_snaps.push(BoardSnapshot {
                id: "blackboard".to_string(),
                pages,
            });
        }
        let active_board_id = active_mode
            .unwrap_or_else(|| "transparent".to_string())
            .to_lowercase();
        SessionSnapshot {
            active_board_id,
            boards: board_snaps,
            tool_state,
        }
    };

    enforce_shape_limits(&mut snapshot, options.max_shapes_per_frame);
    let disk_history_limit = if options.persist_history {
        options.max_persisted_undo_depth
    } else {
        Some(0)
    };
    for board in &mut snapshot.boards {
        apply_history_policies(&mut board.pages, &board.id, disk_history_limit);
    }

    if snapshot.is_empty() && snapshot.tool_state.is_none() {
        debug!(
            "Loaded session file at {} but it contained no data",
            session_path.display()
        );
        return Ok(None);
    }

    Ok(Some(LoadedSnapshot {
        snapshot,
        compressed,
        version: session_file.version,
    }))
}

fn backup_corrupt_session(session_path: &Path, options: &SessionOptions) -> Result<()> {
    let bytes = fs::read(session_path)
        .with_context(|| format!("failed to read corrupt session {}", session_path.display()))?;
    let backup_path = options.backup_file_path();
    fs::write(&backup_path, &bytes)
        .with_context(|| format!("failed to write session backup {}", backup_path.display()))?;
    fs::remove_file(session_path).with_context(|| {
        format!(
            "failed to remove corrupt session {}",
            session_path.display()
        )
    })?;
    Ok(())
}
