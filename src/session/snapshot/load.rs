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
use crate::session::lock::{
    lock_shared, open_existing_runtime_lock_file_for_read, open_runtime_lock_file, unlock,
};
use crate::session::options::SessionOptions;
use crate::session::primary::{
    is_non_regular_session_artifact, open_session_artifact_for_read, session_artifact_metadata,
    session_artifact_metadata_if_exists,
};
use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

mod corrupt;
mod fallback;
mod markers;
mod named_candidate;
mod payload;

use corrupt::backup_corrupt_session;
use fallback::load_normal_session_or_empty;
use markers::{
    backup_is_newer_than_primary, clear_marker_metadata, clear_marker_suppresses_artifact,
    preserve_unloadable_recovery, recoverable_backup_marker_metadata,
    recoverable_recovery_marker_metadata, should_prefer_recovery,
};
use named_candidate::{load_named_candidate_with_fallbacks, log_named_candidate_outcome};
use payload::load_snapshot_opened_with_expanded_limit;

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
    LoadedFromBackup(Box<SessionSnapshot>),
    LoadedFromRecovery(Box<SessionSnapshot>),
    Empty,
    NonRegularArtifact {
        path: PathBuf,
    },
    ExpandedTooLarge {
        path: PathBuf,
        max_expanded_size: u64,
    },
}

impl LoadSnapshotOutcome {
    #[allow(dead_code)]
    pub(crate) fn has_board_data(&self) -> bool {
        match self {
            Self::Loaded(snapshot)
            | Self::LoadedFromBackup(snapshot)
            | Self::LoadedFromRecovery(snapshot) => snapshot.has_board_data(),
            Self::Empty | Self::NonRegularArtifact { .. } | Self::ExpandedTooLarge { .. } => false,
        }
    }
}

/// Attempt to load a previously saved session.
#[allow(dead_code)]
pub fn load_snapshot(options: &SessionOptions) -> Result<Option<SessionSnapshot>> {
    match load_snapshot_with_outcome(options)? {
        LoadSnapshotOutcome::Loaded(snapshot)
        | LoadSnapshotOutcome::LoadedFromBackup(snapshot)
        | LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => Ok(Some(*snapshot)),
        LoadSnapshotOutcome::Empty
        | LoadSnapshotOutcome::NonRegularArtifact { .. }
        | LoadSnapshotOutcome::ExpandedTooLarge { .. } => Ok(None),
    }
}

pub(crate) fn load_snapshot_with_outcome(options: &SessionOptions) -> Result<LoadSnapshotOutcome> {
    load_snapshot_with_expanded_limit(options, DEFAULT_MAX_EXPANDED_SESSION_BYTES)
}

pub(crate) fn load_snapshot_for_offline_edit(
    options: &SessionOptions,
) -> Result<LoadSnapshotOutcome> {
    load_snapshot_with_expanded_limit_inner(options, DEFAULT_MAX_EXPANDED_SESSION_BYTES)
}

#[allow(dead_code)]
pub(crate) fn load_named_session_candidate(
    options: &SessionOptions,
) -> Result<LoadSnapshotOutcome> {
    load_named_session_candidate_with_expanded_limit(options, DEFAULT_MAX_EXPANDED_SESSION_BYTES)
}

#[allow(dead_code)]
pub(super) fn load_named_session_candidate_with_expanded_limit(
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<LoadSnapshotOutcome> {
    if !options.is_named_file() {
        return Err(anyhow!(
            "runtime open requires a named session file target, got configured target {}",
            options.session_file_path().display()
        ));
    }

    let lock_path = options.lock_file_path();
    let lock_file =
        open_existing_runtime_lock_file_for_read(&lock_path, true).with_context(|| {
            format!(
                "failed to inspect session lock file {}",
                lock_path.display()
            )
        })?;
    if let Some(lock_file) = lock_file.as_ref() {
        lock_shared(lock_file)
            .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;
    }

    let session_path = options.session_file_path();
    crate::session::validate_named_session_file_for_open(&session_path)?;

    let result = load_named_candidate_with_fallbacks(&session_path, options, max_expanded_size);

    if let Some(lock_file) = lock_file.as_ref()
        && let Err(err) = unlock(lock_file)
    {
        warn!(
            "failed to unlock session file {}: {}",
            lock_path.display(),
            err
        );
    }

    match result {
        Ok(outcome) => {
            log_named_candidate_outcome(&session_path, &outcome);
            Ok(outcome)
        }
        Err(err) => Err(err).with_context(|| {
            format!(
                "failed to load session candidate {}",
                session_path.display()
            )
        }),
    }
}

pub(super) fn load_snapshot_with_expanded_limit(
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<LoadSnapshotOutcome> {
    let outcome = load_snapshot_with_expanded_limit_inner(options, max_expanded_size)?;
    record_named_session_opened_for_outcome(options, &outcome);
    Ok(outcome)
}

fn load_snapshot_with_expanded_limit_inner(
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
    let (session_metadata, non_regular_primary_path) = match initial_session_metadata(
        &session_path,
        options,
    ) {
        Ok(metadata) => (metadata, None),
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Primary session {} is not a regular file; checking recovery before refusing to load it: {}",
                session_path.display(),
                err
            );
            (None, Some(session_path.clone()))
        }
        Err(err) => return Err(err),
    };
    let recovery_metadata = fs::metadata(&recovery_path).ok();
    let clear_marker_metadata = clear_marker_metadata(options);
    let backup_recovery_marker_metadata =
        recoverable_backup_marker_metadata(options, clear_marker_metadata.as_ref());
    let recovery_recoverable_marker_metadata =
        recoverable_recovery_marker_metadata(options, clear_marker_metadata.as_ref());

    if let Some(recovery_metadata) = recovery_metadata.as_ref()
        && should_prefer_recovery(recovery_metadata, session_metadata.as_ref())
    {
        if clear_marker_suppresses_artifact(
            "session recovery",
            &recovery_path,
            recovery_metadata,
            clear_marker_metadata.as_ref(),
        ) {
            if let Some(path) = non_regular_primary_path.as_ref() {
                return Ok(LoadSnapshotOutcome::NonRegularArtifact { path: path.clone() });
            }
            return load_normal_session_or_empty(
                options,
                &session_path,
                session_metadata,
                max_expanded_size,
                clear_marker_metadata.as_ref(),
                backup_recovery_marker_metadata.as_ref(),
                recovery_recoverable_marker_metadata.as_ref(),
            );
        }
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
            CorruptLoadAction::Backup,
        )?;
        match recovery_outcome {
            LoadSnapshotOutcome::Loaded(snapshot) => {
                return Ok(LoadSnapshotOutcome::LoadedFromRecovery(snapshot));
            }
            loaded @ LoadSnapshotOutcome::LoadedFromBackup(_) => return Ok(loaded),
            loaded @ LoadSnapshotOutcome::LoadedFromRecovery(_) => return Ok(loaded),
            LoadSnapshotOutcome::Empty => {
                warn!(
                    "Session recovery artifact {} did not contain usable session data; falling back to normal session {}",
                    recovery_path.display(),
                    session_path.display()
                );
                preserve_unloadable_recovery(&recovery_path, "empty");
            }
            LoadSnapshotOutcome::NonRegularArtifact { path } => {
                warn!(
                    "Session recovery artifact {} is not a regular file; falling back to normal session {}",
                    path.display(),
                    session_path.display()
                );
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

    if let Some(path) = non_regular_primary_path {
        return Ok(LoadSnapshotOutcome::NonRegularArtifact { path });
    }

    load_normal_session_or_empty(
        options,
        &session_path,
        session_metadata,
        max_expanded_size,
        clear_marker_metadata.as_ref(),
        backup_recovery_marker_metadata.as_ref(),
        recovery_recoverable_marker_metadata.as_ref(),
    )
}

fn initial_session_metadata(
    session_path: &Path,
    options: &SessionOptions,
) -> Result<Option<fs::Metadata>> {
    session_artifact_metadata_if_exists(session_path, is_named_primary_path(session_path, options))
}

fn is_named_primary_path(session_path: &Path, options: &SessionOptions) -> bool {
    options.is_named_file() && session_path == options.session_file_path().as_path()
}

fn load_snapshot_path_with_outcome(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
    enforce_configured_file_size: bool,
    label: &str,
    corrupt_load_action: CorruptLoadAction,
) -> Result<LoadSnapshotOutcome> {
    let no_follow = is_named_primary_path(session_path, options);
    let metadata = match session_artifact_metadata(session_path, no_follow) {
        Ok(metadata) => metadata,
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Refusing to load non-regular {} {}; continuing with defaults: {}",
                label,
                session_path.display(),
                err
            );
            return Ok(LoadSnapshotOutcome::NonRegularArtifact {
                path: session_path.to_path_buf(),
            });
        }
        Err(err) => return Err(err),
    };
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
    let lock_file = open_runtime_lock_file(&lock_path, options.is_named_file())
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
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Refusing to load non-regular {} {}; continuing with defaults: {}",
                label,
                session_path.display(),
                err
            );
            Ok(LoadSnapshotOutcome::NonRegularArtifact {
                path: session_path.to_path_buf(),
            })
        }
        Err(err) => {
            warn!(
                "Failed to load {} {}; continuing with defaults: {}",
                label,
                session_path.display(),
                err
            );
            match corrupt_load_action {
                CorruptLoadAction::Backup => {
                    if let Err(backup_err) = backup_corrupt_session(session_path, options) {
                        warn!(
                            "Failed to back up corrupt session {}: {}",
                            session_path.display(),
                            backup_err
                        );
                    }
                }
                CorruptLoadAction::Preserve => {
                    debug!(
                        "Leaving unloadable {} {} in place because it is suppressed by the session clear marker",
                        label,
                        session_path.display()
                    );
                }
            }
            Ok(LoadSnapshotOutcome::Empty)
        }
    }
}

fn record_named_session_opened_for_outcome(
    options: &SessionOptions,
    outcome: &LoadSnapshotOutcome,
) {
    if options.is_named_file()
        && matches!(
            outcome,
            LoadSnapshotOutcome::Loaded(_)
                | LoadSnapshotOutcome::LoadedFromBackup(_)
                | LoadSnapshotOutcome::LoadedFromRecovery(_)
        )
    {
        crate::session::catalog::record_named_session_opened(options);
    }
}

#[derive(Clone, Copy)]
enum CorruptLoadAction {
    Backup,
    Preserve,
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
    let no_follow = is_named_primary_path(session_path, options);
    let file = open_session_artifact_for_read(session_path, no_follow)?;
    load_snapshot_opened_with_expanded_limit(session_path, options, file, max_expanded_size, None)
}
