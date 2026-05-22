use super::compression::{
    DEFAULT_MAX_EXPANDED_SESSION_BYTES, ExpandedSessionTooLarge, maybe_decompress_with_limit,
};
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
use std::path::{Path, PathBuf};

pub struct LoadedSnapshot {
    pub snapshot: SessionSnapshot,
    pub compressed: bool,
    pub version: u32,
}

/// High-level load result used by runtime callers that need to distinguish a
/// missing session from a protected session that was intentionally left intact.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum LoadSnapshotOutcome {
    Loaded(Box<SessionSnapshot>),
    LoadedFromRecovery(Box<SessionSnapshot>),
    Empty,
    ExpandedTooLarge {
        path: PathBuf,
        max_expanded_size: u64,
    },
}

/// Attempt to load a previously saved session.
#[allow(dead_code)]
pub fn load_snapshot(options: &SessionOptions) -> Result<Option<SessionSnapshot>> {
    match load_snapshot_with_outcome(options)? {
        LoadSnapshotOutcome::Loaded(snapshot)
        | LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => Ok(Some(*snapshot)),
        LoadSnapshotOutcome::Empty | LoadSnapshotOutcome::ExpandedTooLarge { .. } => Ok(None),
    }
}

pub(crate) fn load_snapshot_with_outcome(options: &SessionOptions) -> Result<LoadSnapshotOutcome> {
    load_snapshot_with_expanded_limit(options, DEFAULT_MAX_EXPANDED_SESSION_BYTES)
}

pub(super) fn load_snapshot_with_expanded_limit(
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<LoadSnapshotOutcome> {
    if !options.any_enabled() && !options.restore_tool_state {
        info!(
            "Session load skipped: persistence disabled (base_dir={}, file={})",
            options.base_dir.display(),
            options.session_file_path().display()
        );
        return Ok(LoadSnapshotOutcome::Empty);
    }

    let session_path = options.session_file_path();
    let recovery_path = options.recovery_file_path();
    let session_metadata = fs::metadata(&session_path).ok();
    let recovery_metadata = fs::metadata(&recovery_path).ok();

    if let Some(recovery_metadata) = recovery_metadata.as_ref()
        && should_prefer_recovery(recovery_metadata, session_metadata.as_ref())
    {
        info!(
            "Loading session recovery artifact {} before normal session {}",
            recovery_path.display(),
            session_path.display()
        );
        let recovery_outcome = load_snapshot_path_with_outcome(
            &recovery_path,
            options,
            max_expanded_size,
            false,
            "session recovery",
        )?;
        match recovery_outcome {
            LoadSnapshotOutcome::Loaded(snapshot) => {
                return Ok(LoadSnapshotOutcome::LoadedFromRecovery(snapshot));
            }
            loaded @ LoadSnapshotOutcome::LoadedFromRecovery(_) => return Ok(loaded),
            LoadSnapshotOutcome::Empty => {
                warn!(
                    "Session recovery artifact {} did not contain usable session data; falling back to normal session {}",
                    recovery_path.display(),
                    session_path.display()
                );
                preserve_unloadable_recovery(&recovery_path, "empty");
            }
            LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => {
                warn!(
                    "Session recovery artifact {} exceeded the expanded load safety cap; preserving it and falling back to normal session {}",
                    path.display(),
                    session_path.display()
                );
                preserve_unloadable_recovery(&path, "too-large");
            }
        }
    }

    load_normal_session_or_empty(options, &session_path, session_metadata, max_expanded_size)
}

fn load_snapshot_path_with_outcome(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
    enforce_configured_file_size: bool,
    label: &str,
) -> Result<LoadSnapshotOutcome> {
    let metadata = fs::metadata(session_path)
        .with_context(|| format!("failed to stat session file {}", session_path.display()))?;
    info!(
        "{} file present at {} ({} bytes, per_output={}, output_identity={:?})",
        label,
        session_path.display(),
        metadata.len(),
        options.per_output,
        options.output_identity()
    );
    if enforce_configured_file_size && metadata.len() > options.max_file_size_bytes {
        warn!(
            "Session file {} is {} bytes which exceeds the configured limit ({} bytes); refusing to load",
            session_path.display(),
            metadata.len(),
            options.max_file_size_bytes
        );
        return Ok(LoadSnapshotOutcome::Empty);
    } else if !enforce_configured_file_size {
        if metadata.len() > max_expanded_size {
            warn!(
                "Session recovery file {} is {} bytes which exceeds the expanded load safety limit ({} bytes); refusing to read",
                session_path.display(),
                metadata.len(),
                max_expanded_size
            );
            return Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                path: session_path.to_path_buf(),
                max_expanded_size,
            });
        }
        if metadata.len() > options.max_file_size_bytes {
            info!(
                "Session recovery file {} is {} bytes, above configured normal session limit {}; loading with expanded safety cap only",
                session_path.display(),
                metadata.len(),
                options.max_file_size_bytes
            );
        }
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

    let result = load_snapshot_inner_with_expanded_limit(session_path, options, max_expanded_size);

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
                "Loaded {} from {} (version {}, compressed={}, boards={}, active_board={}, tool_state={})",
                label,
                session_path.display(),
                loaded.version,
                loaded.compressed,
                loaded.snapshot.boards.len(),
                loaded.snapshot.active_board_id,
                tool_state
            );
            Ok(LoadSnapshotOutcome::Loaded(Box::new(loaded.snapshot)))
        }
        Ok(None) => {
            info!(
                "{} file {} contained no usable data; continuing with defaults",
                label,
                session_path.display()
            );
            Ok(LoadSnapshotOutcome::Empty)
        }
        Err(err) if err.downcast_ref::<ExpandedSessionTooLarge>().is_some() => {
            warn!(
                "Refusing to load session {}; expanded payload exceeds safety limit ({} bytes). The session file is left unchanged; clear the session or move the file if it is no longer needed: {}",
                session_path.display(),
                max_expanded_size,
                err
            );
            Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                path: session_path.to_path_buf(),
                max_expanded_size,
            })
        }
        Err(err) => {
            warn!(
                "Failed to load {} {}; backing up and continuing with defaults: {}",
                label,
                session_path.display(),
                err
            );
            if let Err(backup_err) = backup_corrupt_session(session_path, options) {
                warn!(
                    "Failed to back up corrupt session {}: {}",
                    session_path.display(),
                    backup_err
                );
            }
            Ok(LoadSnapshotOutcome::Empty)
        }
    }
}

fn load_normal_session_or_empty(
    options: &SessionOptions,
    session_path: &Path,
    session_metadata: Option<fs::Metadata>,
    max_expanded_size: u64,
) -> Result<LoadSnapshotOutcome> {
    if session_metadata.is_none() {
        info!(
            "Session file not found at {}; skipping load",
            session_path.display()
        );
        return Ok(LoadSnapshotOutcome::Empty);
    }
    load_snapshot_path_with_outcome(session_path, options, max_expanded_size, true, "session")
}

fn preserve_unloadable_recovery(path: &Path, reason: &str) {
    if !path.exists() {
        return;
    }
    let preserved_path = preserved_recovery_path(path, reason);
    match fs::rename(path, &preserved_path) {
        Ok(()) => warn!(
            "Preserved unloadable session recovery artifact as {}",
            preserved_path.display()
        ),
        Err(err) => warn!(
            "Failed to preserve unloadable session recovery artifact {}: {}",
            path.display(),
            err
        ),
    }
}

fn preserved_recovery_path(path: &Path, reason: &str) -> PathBuf {
    let base_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .map_or_else(
            || reason.to_string(),
            |extension| format!("{extension}.{reason}"),
        );

    for index in 0..100 {
        let extension = if index == 0 {
            base_extension.clone()
        } else {
            format!("{base_extension}.{index}")
        };
        let candidate = path.with_extension(extension);
        if !candidate.exists() {
            return candidate;
        }
    }

    path.with_extension(format!("{base_extension}.100"))
}

fn should_prefer_recovery(
    recovery_metadata: &fs::Metadata,
    session_metadata: Option<&fs::Metadata>,
) -> bool {
    let Some(session_metadata) = session_metadata else {
        return true;
    };
    match (recovery_metadata.modified(), session_metadata.modified()) {
        (Ok(recovery_modified), Ok(session_modified)) => recovery_modified >= session_modified,
        _ => true,
    }
}

pub(crate) fn load_snapshot_inner(
    session_path: &Path,
    options: &SessionOptions,
) -> Result<Option<LoadedSnapshot>> {
    load_snapshot_inner_with_expanded_limit(
        session_path,
        options,
        DEFAULT_MAX_EXPANDED_SESSION_BYTES,
    )
}

pub(super) fn load_snapshot_inner_with_expanded_limit(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<LoadedSnapshot>> {
    let mut file_bytes = Vec::new();
    {
        let mut file = File::open(session_path)
            .with_context(|| format!("failed to open session file {}", session_path.display()))?;
        file.read_to_end(&mut file_bytes)
            .context("failed to read session file")?;
    }

    let (decompressed, compressed) = maybe_decompress_with_limit(file_bytes, max_expanded_size)?;

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

    let mut snapshot = if !boards.is_empty() || active_board_id.is_some() {
        let mut board_snaps = Vec::new();
        for BoardFile {
            id,
            pages,
            active_page,
        } in boards
        {
            board_snaps.push(BoardSnapshot {
                id,
                pages: normalized_board_pages_snapshot(pages, Some(active_page)),
            });
        }
        let active_board_id = resolved_active_board_id(active_board_id, &board_snaps);
        SessionSnapshot {
            active_board_id,
            boards: board_snaps,
            tool_state,
        }
    } else {
        let mut board_snaps = Vec::new();
        if let Some(pages) =
            board_pages_from_file(transparent_pages, transparent_active_page, transparent)
        {
            board_snaps.push(BoardSnapshot {
                id: "transparent".to_string(),
                pages,
            });
        }
        if let Some(pages) =
            board_pages_from_file(whiteboard_pages, whiteboard_active_page, whiteboard)
        {
            board_snaps.push(BoardSnapshot {
                id: "whiteboard".to_string(),
                pages,
            });
        }
        if let Some(pages) =
            board_pages_from_file(blackboard_pages, blackboard_active_page, blackboard)
        {
            board_snaps.push(BoardSnapshot {
                id: "blackboard".to_string(),
                pages,
            });
        }
        let active_board_id =
            resolved_active_board_id(active_mode.map(|mode| mode.to_lowercase()), &board_snaps);
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

fn board_pages_from_file(
    pages: Option<Vec<Frame>>,
    active: Option<usize>,
    legacy: Option<Frame>,
) -> Option<BoardPagesSnapshot> {
    if let Some(pages) = pages {
        return Some(normalized_board_pages_snapshot(pages, active));
    }
    legacy.map(|frame| BoardPagesSnapshot {
        pages: vec![frame],
        active: 0,
    })
}

fn normalized_board_pages_snapshot(
    mut pages: Vec<Frame>,
    active: Option<usize>,
) -> BoardPagesSnapshot {
    if pages.is_empty() {
        pages.push(Frame::new());
    }
    let active = active.unwrap_or(0).min(pages.len().saturating_sub(1));
    BoardPagesSnapshot { pages, active }
}

fn resolved_active_board_id(requested: Option<String>, boards: &[BoardSnapshot]) -> String {
    let Some(fallback_id) = boards.first().map(|board| board.id.clone()) else {
        return "transparent".to_string();
    };

    let requested = requested.unwrap_or_else(|| fallback_id.clone());
    if boards.iter().any(|board| board.id == requested) {
        requested
    } else {
        warn!(
            "Session active board '{}' missing from restored boards; falling back to '{}'",
            requested, fallback_id
        );
        fallback_id
    }
}

fn backup_corrupt_session(session_path: &Path, options: &SessionOptions) -> Result<()> {
    let bytes = fs::read(session_path)
        .with_context(|| format!("failed to read corrupt session {}", session_path.display()))?;
    let primary_path = options.session_file_path();
    let backup_path = if session_path == primary_path.as_path() {
        options.backup_file_path()
    } else {
        session_path.with_extension("recovery.bak")
    };
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
