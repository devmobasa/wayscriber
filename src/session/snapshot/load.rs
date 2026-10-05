use super::compression::{
    DEFAULT_MAX_EXPANDED_SESSION_BYTES, ExpandedSessionTooLarge, is_gzip,
    maybe_decompress_with_limit,
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
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

mod artifact_outcome;
mod corrupt;
mod fallback;
mod markers;
mod named_candidate;
mod payload;

pub(crate) use artifact_outcome::CorruptArtifactPreservationFailed;
use artifact_outcome::{CorruptLoadAction, artifact_load_outcome};
use corrupt::{preserve_newer_version_session, quarantine_corrupt_artifact};
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
    RestoredAfterCorruption {
        snapshot: Box<SessionSnapshot>,
        source: RestoredArtifact,
        corrupt_copy: PathBuf,
    },
    Empty,
    /// Nothing was restored because the stored session could not be read. Its
    /// bytes are preserved at `corrupt_copy`; the caller is expected to say so
    /// rather than let a silent empty canvas stand in for lost drawings.
    EmptyAfterCorruption {
        corrupt_copy: PathBuf,
    },
    NonRegularArtifact {
        path: PathBuf,
    },
    ExpandedTooLarge {
        path: PathBuf,
        max_expanded_size: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestoredArtifact {
    Backup,
    Recovery,
}

/// A too-new session could be read, but no durable copy could be established.
///
/// This is deliberately distinct from a malformed session: the generic load
/// error path backs malformed files up and then continues with an empty
/// session, which would expose this valid newer file to the save/rotation cycle
/// preservation exists to prevent.
#[derive(Debug)]
struct NewerVersionPreservationFailed {
    path: PathBuf,
    details: String,
}

impl fmt::Display for NewerVersionPreservationFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not durably preserve newer-version session {}: {}",
            self.path.display(),
            self.details
        )
    }
}

impl std::error::Error for NewerVersionPreservationFailed {}

impl LoadSnapshotOutcome {
    #[allow(dead_code)]
    pub(crate) fn has_board_data(&self) -> bool {
        match self {
            Self::Loaded(snapshot)
            | Self::LoadedFromBackup(snapshot)
            | Self::RestoredAfterCorruption { snapshot, .. }
            | Self::LoadedFromRecovery(snapshot) => snapshot.has_board_data(),
            Self::Empty
            | Self::EmptyAfterCorruption { .. }
            | Self::NonRegularArtifact { .. }
            | Self::ExpandedTooLarge { .. } => false,
        }
    }
}

/// Attempt to load a previously saved session.
pub fn load_snapshot(options: &SessionOptions) -> Result<Option<SessionSnapshot>> {
    match load_snapshot_with_outcome(options)? {
        LoadSnapshotOutcome::Loaded(snapshot)
        | LoadSnapshotOutcome::LoadedFromBackup(snapshot)
        | LoadSnapshotOutcome::RestoredAfterCorruption { snapshot, .. }
        | LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => Ok(Some(*snapshot)),
        LoadSnapshotOutcome::Empty
        | LoadSnapshotOutcome::EmptyAfterCorruption { .. }
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
    load_snapshot_with_expanded_limit_inner(options, DEFAULT_MAX_EXPANDED_SESSION_BYTES, false)
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
    let outcome = load_snapshot_with_expanded_limit_inner(options, max_expanded_size, true)?;
    record_named_session_opened_for_outcome(options, &outcome);
    Ok(outcome)
}

fn load_snapshot_with_expanded_limit_inner(
    options: &SessionOptions,
    max_expanded_size: u64,
    restore_named_primary: bool,
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
                restore_named_primary,
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
            CorruptLoadAction::Quarantine,
        )?;
        match recovery_outcome {
            LoadSnapshotOutcome::Loaded(snapshot) => {
                return Ok(LoadSnapshotOutcome::LoadedFromRecovery(snapshot));
            }
            loaded @ LoadSnapshotOutcome::RestoredAfterCorruption { .. } => return Ok(loaded),
            loaded @ LoadSnapshotOutcome::LoadedFromBackup(_) => return Ok(loaded),
            loaded @ LoadSnapshotOutcome::LoadedFromRecovery(_) => return Ok(loaded),
            LoadSnapshotOutcome::Empty | LoadSnapshotOutcome::EmptyAfterCorruption { .. } => {
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
        restore_named_primary,
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
    if let Some(outcome) = reject_oversized_snapshot(
        session_path,
        metadata.len(),
        options,
        max_expanded_size,
        enforce_configured_file_size,
    ) {
        return Ok(outcome);
    }

    with_shared_session_lock(options, || {
        let result =
            load_snapshot_inner_with_expanded_limit(session_path, options, max_expanded_size);
        artifact_load_outcome(
            result,
            session_path,
            options,
            max_expanded_size,
            label,
            corrupt_load_action,
        )
    })
}

fn with_shared_session_lock<T>(
    options: &SessionOptions,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let lock_path = options.lock_file_path();
    let file = open_runtime_lock_file(&lock_path, options.is_named_file())
        .with_context(|| format!("failed to open session lock file {}", lock_path.display()))?;
    lock_shared(&file)
        .with_context(|| format!("failed to acquire shared lock {}", lock_path.display()))?;
    let result = operation();
    if let Err(err) = unlock(&file) {
        warn!("failed to unlock session {}: {err}", lock_path.display());
    }
    result
}

fn reject_oversized_snapshot(
    session_path: &Path,
    file_size: u64,
    options: &SessionOptions,
    max_expanded_size: u64,
    enforce_configured_file_size: bool,
) -> Option<LoadSnapshotOutcome> {
    if enforce_configured_file_size && file_size > options.max_file_size_bytes {
        warn!(
            "Session file {} is {} bytes which exceeds the configured limit ({} bytes); refusing to load",
            session_path.display(),
            file_size,
            options.max_file_size_bytes
        );
        return Some(LoadSnapshotOutcome::Empty);
    }
    if enforce_configured_file_size {
        return None;
    }
    if file_size > max_expanded_size {
        warn!(
            "Session recovery file {} is {} bytes which exceeds the expanded load safety limit ({} bytes); refusing to read",
            session_path.display(),
            file_size,
            max_expanded_size
        );
        return Some(LoadSnapshotOutcome::ExpandedTooLarge {
            path: session_path.to_path_buf(),
            max_expanded_size,
        });
    }
    if file_size > options.max_file_size_bytes {
        info!(
            "Session recovery file {} is {} bytes, above configured normal session limit {}; loading with expanded safety cap only",
            session_path.display(),
            file_size,
            options.max_file_size_bytes
        );
    }
    None
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
                | LoadSnapshotOutcome::RestoredAfterCorruption { .. }
        )
    {
        crate::session::catalog::record_named_session_opened(options);
    }
}

/// What to do with a session file written by a newer wayscriber than this one.
#[derive(Clone, Copy)]
enum NewerVersionAction {
    /// Runtime load of the active session: preserve a copy under a versioned
    /// side name first, because the empty session this load returns will be
    /// saved over the file — and with `backup_retention: 1` the second rotation
    /// destroys the newer-version data for good.
    Preserve,
    /// Read-only candidate inspection: must not create or mutate any artifact.
    LeaveUntouched,
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
    load_snapshot_opened_with_expanded_limit(
        session_path,
        options,
        file,
        max_expanded_size,
        None,
        NewerVersionAction::Preserve,
    )
}
