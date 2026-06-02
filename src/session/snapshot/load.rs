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

fn log_named_candidate_outcome(session_path: &Path, outcome: &LoadSnapshotOutcome) {
    let (source, snapshot) = match outcome {
        LoadSnapshotOutcome::Loaded(snapshot) => ("primary", snapshot),
        LoadSnapshotOutcome::LoadedFromBackup(snapshot) => ("backup", snapshot),
        LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => ("recovery", snapshot),
        LoadSnapshotOutcome::Empty
        | LoadSnapshotOutcome::NonRegularArtifact { .. }
        | LoadSnapshotOutcome::ExpandedTooLarge { .. } => return,
    };
    info!(
        "Loaded named session candidate {} from {} (boards={}, active_board={}, tool_state={})",
        source,
        session_path.display(),
        snapshot.boards.len(),
        snapshot.active_board_id,
        snapshot.tool_state.is_some()
    );
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

fn load_normal_session_or_empty(
    options: &SessionOptions,
    session_path: &Path,
    session_metadata: Option<fs::Metadata>,
    max_expanded_size: u64,
    clear_marker_metadata: Option<&fs::Metadata>,
    backup_recovery_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<LoadSnapshotOutcome> {
    let Some(primary_metadata) = session_metadata.as_ref() else {
        if let Some(backup) = load_contentful_backup(
            options,
            max_expanded_size,
            None,
            clear_marker_metadata,
            backup_recovery_marker_metadata,
            recovery_recoverable_marker_metadata,
        )? {
            return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
        }
        info!(
            "Session file not found at {}; skipping load",
            session_path.display()
        );
        return Ok(LoadSnapshotOutcome::Empty);
    };

    if clear_marker_suppresses_artifact(
        "primary session",
        session_path,
        primary_metadata,
        clear_marker_metadata,
    ) {
        match load_snapshot_path_with_outcome(
            session_path,
            options,
            max_expanded_size,
            true,
            "session",
            CorruptLoadAction::Preserve,
        )? {
            LoadSnapshotOutcome::Loaded(snapshot) if !snapshot.has_board_data() => {
                return Ok(LoadSnapshotOutcome::Loaded(snapshot));
            }
            LoadSnapshotOutcome::Loaded(_) => {}
            LoadSnapshotOutcome::Empty
            | LoadSnapshotOutcome::NonRegularArtifact { .. }
            | LoadSnapshotOutcome::ExpandedTooLarge { .. } => {}
            LoadSnapshotOutcome::LoadedFromBackup(_)
            | LoadSnapshotOutcome::LoadedFromRecovery(_) => {}
        }

        if let Some(backup) = load_contentful_backup(
            options,
            max_expanded_size,
            None,
            clear_marker_metadata,
            backup_recovery_marker_metadata,
            recovery_recoverable_marker_metadata,
        )? {
            return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
        }
        return Ok(LoadSnapshotOutcome::Empty);
    }

    let outcome = load_snapshot_path_with_outcome(
        session_path,
        options,
        max_expanded_size,
        true,
        "session",
        CorruptLoadAction::Backup,
    )?;
    let LoadSnapshotOutcome::Loaded(snapshot) = outcome else {
        return Ok(outcome);
    };

    if snapshot.has_board_data() {
        return Ok(LoadSnapshotOutcome::Loaded(snapshot));
    }

    if let Some(backup) = load_contentful_backup(
        options,
        max_expanded_size,
        Some(primary_metadata),
        clear_marker_metadata,
        backup_recovery_marker_metadata,
        recovery_recoverable_marker_metadata,
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
    }

    if let Some(recovery) = load_contentful_recovery(
        options,
        max_expanded_size,
        clear_marker_metadata,
        recovery_recoverable_marker_metadata,
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromRecovery(recovery));
    }

    Ok(LoadSnapshotOutcome::Loaded(snapshot))
}

fn load_contentful_backup(
    options: &SessionOptions,
    max_expanded_size: u64,
    primary_metadata: Option<&fs::Metadata>,
    clear_marker_metadata: Option<&fs::Metadata>,
    backup_recovery_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    let backup_path = options.backup_file_path();
    let Some(backup_metadata) = fs::metadata(&backup_path).ok() else {
        return Ok(None);
    };

    if clear_marker_suppresses_artifact(
        "session backup",
        &backup_path,
        &backup_metadata,
        clear_marker_metadata,
    ) {
        return Ok(None);
    }

    if let Some(primary_metadata) = primary_metadata
        && backup_recovery_marker_metadata.is_none()
        && !backup_is_newer_than_primary(&backup_metadata, primary_metadata)
    {
        info!(
            "Primary session {} contains no board data, but backup {} is not newer; keeping primary session",
            options.session_file_path().display(),
            backup_path.display()
        );
        return Ok(None);
    }

    if primary_metadata.is_some() {
        warn!(
            "Primary session {} contains no board data; checking newer backup {} before accepting the blank session",
            options.session_file_path().display(),
            backup_path.display()
        );
    } else {
        warn!(
            "Primary session {} is missing; checking backup {} for recoverable board data",
            options.session_file_path().display(),
            backup_path.display()
        );
    }

    match load_snapshot_path_with_outcome(
        &backup_path,
        options,
        max_expanded_size,
        true,
        "session backup",
        CorruptLoadAction::Backup,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => {
            warn!(
                "Restoring board data from backup {} because primary session {} contained no board data",
                backup_path.display(),
                options.session_file_path().display()
            );
            Ok(Some(snapshot))
        }
        LoadSnapshotOutcome::Loaded(_) => {
            info!(
                "Session backup {} also contains no board data; keeping primary session",
                backup_path.display()
            );
            Ok(None)
        }
        LoadSnapshotOutcome::Empty | LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => {
            warn!(
                "Session backup {} is too large to restore; keeping primary session",
                path.display()
            );
            Ok(None)
        }
        LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_) => {
            load_contentful_recovery(
                options,
                max_expanded_size,
                clear_marker_metadata,
                recovery_recoverable_marker_metadata,
            )
        }
    }
}

fn load_contentful_recovery(
    options: &SessionOptions,
    max_expanded_size: u64,
    clear_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    if recovery_recoverable_marker_metadata.is_none() {
        return Ok(None);
    }

    let recovery_path = options.recovery_file_path();
    let Some(recovery_metadata) = fs::metadata(&recovery_path).ok() else {
        return Ok(None);
    };
    if clear_marker_suppresses_artifact(
        "session recovery",
        &recovery_path,
        &recovery_metadata,
        clear_marker_metadata,
    ) {
        return Ok(None);
    }

    match load_snapshot_path_with_outcome(
        &recovery_path,
        options,
        max_expanded_size,
        false,
        "session recovery",
        CorruptLoadAction::Backup,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => Ok(Some(snapshot)),
        LoadSnapshotOutcome::Loaded(_)
        | LoadSnapshotOutcome::Empty
        | LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => {
            preserve_unloadable_recovery(&path, "too-large");
            Ok(None)
        }
        LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_) => {
            Ok(None)
        }
    }
}

fn backup_is_newer_than_primary(backup: &fs::Metadata, primary: &fs::Metadata) -> bool {
    match (backup.modified(), primary.modified()) {
        (Ok(backup_modified), Ok(primary_modified)) => backup_modified > primary_modified,
        _ => false,
    }
}

fn recoverable_backup_marker_metadata(
    options: &SessionOptions,
    clear_marker_metadata: Option<&fs::Metadata>,
) -> Option<fs::Metadata> {
    let marker_path = options.backup_recovery_marker_file_path();
    let marker_metadata =
        session_marker_metadata_if_exists("backup recovery marker", &marker_path, options)?;
    if clear_marker_suppresses_artifact(
        "backup recovery marker",
        &marker_path,
        &marker_metadata,
        clear_marker_metadata,
    ) {
        return None;
    }
    Some(marker_metadata)
}

fn recoverable_recovery_marker_metadata(
    options: &SessionOptions,
    clear_marker_metadata: Option<&fs::Metadata>,
) -> Option<fs::Metadata> {
    let marker_path = options.recovery_recoverable_marker_file_path();
    let marker_metadata =
        session_marker_metadata_if_exists("recovery recoverable marker", &marker_path, options)?;
    if clear_marker_suppresses_artifact(
        "recovery recoverable marker",
        &marker_path,
        &marker_metadata,
        clear_marker_metadata,
    ) {
        return None;
    }
    Some(marker_metadata)
}

fn clear_marker_metadata(options: &SessionOptions) -> Option<fs::Metadata> {
    let marker_path = options.clear_marker_file_path();
    session_marker_metadata_if_exists("clear marker", &marker_path, options)
}

fn session_marker_metadata_if_exists(
    label: &str,
    path: &Path,
    options: &SessionOptions,
) -> Option<fs::Metadata> {
    match session_artifact_metadata_if_exists(path, options.is_named_file()) {
        Ok(metadata) => metadata,
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Ignoring non-regular session {} {}: {}",
                label,
                path.display(),
                err
            );
            None
        }
        Err(err) => {
            warn!(
                "Ignoring unreadable session {} {}: {:#}",
                label,
                path.display(),
                err
            );
            None
        }
    }
}

fn clear_marker_suppresses_artifact(
    label: &str,
    path: &Path,
    artifact_metadata: &fs::Metadata,
    clear_marker_metadata: Option<&fs::Metadata>,
) -> bool {
    let Some(clear_marker_metadata) = clear_marker_metadata else {
        return false;
    };
    let artifact_newer_than_marker = match (
        artifact_metadata.modified(),
        clear_marker_metadata.modified(),
    ) {
        (Ok(artifact_modified), Ok(marker_modified)) => artifact_modified > marker_modified,
        _ => false,
    };
    if artifact_newer_than_marker {
        return false;
    }
    info!(
        "Ignoring {} {} because it is not newer than the session clear marker",
        label,
        path.display()
    );
    true
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
    for index in 0..100 {
        let suffix = if index == 0 {
            format!(".{reason}")
        } else {
            format!(".{reason}.{index}")
        };
        let candidate = crate::session::append_path_suffix(path, &suffix);
        if !candidate.exists() {
            return candidate;
        }
    }

    crate::session::append_path_suffix(path, &format!(".{reason}.100"))
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
    let no_follow = is_named_primary_path(session_path, options);
    let file = open_session_artifact_for_read(session_path, no_follow)?;
    load_snapshot_opened_with_expanded_limit(session_path, options, file, max_expanded_size, None)
}

#[allow(dead_code)]
fn load_named_candidate_with_fallbacks(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<LoadSnapshotOutcome> {
    let primary_metadata = session_artifact_metadata(session_path, true)?;
    let recovery_path = options.recovery_file_path();
    let recovery_metadata = named_candidate_artifact_metadata_if_exists(&recovery_path)?;
    let clear_marker_metadata = clear_marker_metadata(options);
    let backup_recovery_marker_metadata =
        recoverable_backup_marker_metadata(options, clear_marker_metadata.as_ref());
    let recovery_recoverable_marker_metadata =
        recoverable_recovery_marker_metadata(options, clear_marker_metadata.as_ref());

    if let Some(recovery_metadata) = recovery_metadata.as_ref()
        && should_prefer_recovery(recovery_metadata, Some(&primary_metadata))
        && !clear_marker_suppresses_artifact(
            "session recovery",
            &recovery_path,
            recovery_metadata,
            clear_marker_metadata.as_ref(),
        )
    {
        match load_named_candidate_sidecar_artifact(
            "session recovery",
            &recovery_path,
            options,
            max_expanded_size,
            CandidateSizeLimit::ExpandedOnly,
        )? {
            LoadSnapshotOutcome::Loaded(snapshot) => {
                return Ok(LoadSnapshotOutcome::LoadedFromRecovery(snapshot));
            }
            LoadSnapshotOutcome::ExpandedTooLarge {
                path,
                max_expanded_size,
            } => {
                warn!(
                    "Named session candidate recovery {} is newer but expands beyond the {} byte safety limit; aborting open to preserve the recovery artifact",
                    path.display(),
                    max_expanded_size
                );
                return Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                    path,
                    max_expanded_size,
                });
            }
            LoadSnapshotOutcome::Empty | LoadSnapshotOutcome::NonRegularArtifact { .. } => {}
            LoadSnapshotOutcome::LoadedFromBackup(_)
            | LoadSnapshotOutcome::LoadedFromRecovery(_) => {}
        }
    }

    if clear_marker_suppresses_artifact(
        "primary session",
        session_path,
        &primary_metadata,
        clear_marker_metadata.as_ref(),
    ) {
        if let Some(primary_snapshot) =
            load_named_candidate_suppressed_primary(session_path, options, max_expanded_size)?
        {
            return Ok(LoadSnapshotOutcome::Loaded(primary_snapshot));
        }

        if let Some(backup) = load_named_candidate_contentful_backup(
            options,
            max_expanded_size,
            None,
            clear_marker_metadata.as_ref(),
            backup_recovery_marker_metadata.as_ref(),
        )? {
            return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
        }

        return Ok(LoadSnapshotOutcome::Empty);
    }

    let primary_outcome = load_named_candidate_artifact(
        session_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::Configured,
    )?;
    let LoadSnapshotOutcome::Loaded(primary_snapshot) = primary_outcome else {
        return Ok(primary_outcome);
    };
    if primary_snapshot.has_board_data() {
        return Ok(LoadSnapshotOutcome::Loaded(primary_snapshot));
    }

    if let Some(backup) = load_named_candidate_contentful_backup(
        options,
        max_expanded_size,
        Some(&primary_metadata),
        clear_marker_metadata.as_ref(),
        backup_recovery_marker_metadata.as_ref(),
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromBackup(backup));
    }

    if let Some(recovery) = load_named_candidate_contentful_recovery(
        options,
        max_expanded_size,
        clear_marker_metadata.as_ref(),
        recovery_recoverable_marker_metadata.as_ref(),
    )? {
        return Ok(LoadSnapshotOutcome::LoadedFromRecovery(recovery));
    }

    Ok(LoadSnapshotOutcome::Loaded(primary_snapshot))
}

#[derive(Clone, Copy)]
enum CandidateSizeLimit {
    Configured,
    ExpandedOnly,
}

fn load_named_candidate_artifact(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
    size_limit: CandidateSizeLimit,
) -> Result<LoadSnapshotOutcome> {
    let file = open_session_artifact_for_read(session_path, true)?;
    let metadata = file.metadata().with_context(|| {
        format!(
            "failed to inspect opened session file {}",
            session_path.display()
        )
    })?;
    let max_encoded_size = match size_limit {
        CandidateSizeLimit::Configured => {
            if metadata.len() > options.max_file_size_bytes {
                return Err(anyhow!(
                    "session file {} is {} bytes which exceeds the configured limit of {} bytes",
                    session_path.display(),
                    metadata.len(),
                    options.max_file_size_bytes
                ));
            }
            Some(options.max_file_size_bytes)
        }
        CandidateSizeLimit::ExpandedOnly => {
            if metadata.len() > max_expanded_size {
                return Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                    path: session_path.to_path_buf(),
                    max_expanded_size,
                });
            }
            None
        }
    };

    match load_snapshot_opened_with_expanded_limit(
        session_path,
        options,
        file,
        max_expanded_size,
        max_encoded_size,
    ) {
        Ok(Some(loaded)) => Ok(LoadSnapshotOutcome::Loaded(Box::new(loaded.snapshot))),
        Ok(None) => Ok(LoadSnapshotOutcome::Empty),
        Err(err) if err.downcast_ref::<ExpandedSessionTooLarge>().is_some() => {
            Ok(LoadSnapshotOutcome::ExpandedTooLarge {
                path: session_path.to_path_buf(),
                max_expanded_size,
            })
        }
        Err(err) if is_non_regular_session_artifact(&err) => {
            Ok(LoadSnapshotOutcome::NonRegularArtifact {
                path: session_path.to_path_buf(),
            })
        }
        Err(err) => Err(err),
    }
}

fn load_named_candidate_sidecar_artifact(
    label: &str,
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
    size_limit: CandidateSizeLimit,
) -> Result<LoadSnapshotOutcome> {
    match load_named_candidate_artifact(session_path, options, max_expanded_size, size_limit) {
        Ok(outcome) => Ok(outcome),
        Err(err) => {
            warn!(
                "Ignoring unreadable named session candidate {} {}; falling back without mutating it: {:#}",
                label,
                session_path.display(),
                err
            );
            Ok(LoadSnapshotOutcome::Empty)
        }
    }
}

fn load_named_candidate_suppressed_primary(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<Box<SessionSnapshot>>> {
    match load_named_candidate_artifact(
        session_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::Configured,
    ) {
        Ok(LoadSnapshotOutcome::Loaded(snapshot)) if !snapshot.has_board_data() => {
            Ok(Some(snapshot))
        }
        Ok(LoadSnapshotOutcome::Loaded(_)) => Ok(None),
        Ok(
            LoadSnapshotOutcome::Empty
            | LoadSnapshotOutcome::NonRegularArtifact { .. }
            | LoadSnapshotOutcome::ExpandedTooLarge { .. },
        ) => Ok(None),
        Ok(
            LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_),
        ) => Ok(None),
        Err(err) => {
            warn!(
                "Ignoring unreadable named session candidate primary {} because it is older than the clear marker: {:#}",
                session_path.display(),
                err
            );
            Ok(None)
        }
    }
}

fn named_candidate_artifact_metadata_if_exists(path: &Path) -> Result<Option<fs::Metadata>> {
    match session_artifact_metadata_if_exists(path, true) {
        Ok(metadata) => Ok(metadata),
        Err(err) if is_non_regular_session_artifact(&err) => {
            warn!(
                "Ignoring non-regular named session candidate sidecar {}: {}",
                path.display(),
                err
            );
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

fn load_named_candidate_contentful_backup(
    options: &SessionOptions,
    max_expanded_size: u64,
    primary_metadata: Option<&fs::Metadata>,
    clear_marker_metadata: Option<&fs::Metadata>,
    backup_recovery_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    let backup_path = options.backup_file_path();
    let Some(backup_metadata) = named_candidate_artifact_metadata_if_exists(&backup_path)? else {
        return Ok(None);
    };
    if clear_marker_suppresses_artifact(
        "session backup",
        &backup_path,
        &backup_metadata,
        clear_marker_metadata,
    ) {
        return Ok(None);
    }
    if let Some(primary_metadata) = primary_metadata
        && backup_recovery_marker_metadata.is_none()
        && !backup_is_newer_than_primary(&backup_metadata, primary_metadata)
    {
        return Ok(None);
    }

    match load_named_candidate_sidecar_artifact(
        "session backup",
        &backup_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::Configured,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => Ok(Some(snapshot)),
        LoadSnapshotOutcome::Loaded(_) | LoadSnapshotOutcome::Empty => Ok(None),
        LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => Err(anyhow!(
            "named session backup is too large to open without mutating candidate artifacts: {}",
            path.display()
        )),
        LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_) => {
            Ok(None)
        }
    }
}

fn load_named_candidate_contentful_recovery(
    options: &SessionOptions,
    max_expanded_size: u64,
    clear_marker_metadata: Option<&fs::Metadata>,
    recovery_recoverable_marker_metadata: Option<&fs::Metadata>,
) -> Result<Option<Box<SessionSnapshot>>> {
    if recovery_recoverable_marker_metadata.is_none() {
        return Ok(None);
    }

    let recovery_path = options.recovery_file_path();
    let Some(recovery_metadata) = named_candidate_artifact_metadata_if_exists(&recovery_path)?
    else {
        return Ok(None);
    };
    if clear_marker_suppresses_artifact(
        "session recovery",
        &recovery_path,
        &recovery_metadata,
        clear_marker_metadata,
    ) {
        return Ok(None);
    }

    match load_named_candidate_sidecar_artifact(
        "session recovery",
        &recovery_path,
        options,
        max_expanded_size,
        CandidateSizeLimit::ExpandedOnly,
    )? {
        LoadSnapshotOutcome::Loaded(snapshot) if snapshot.has_board_data() => Ok(Some(snapshot)),
        LoadSnapshotOutcome::Loaded(_) | LoadSnapshotOutcome::Empty => Ok(None),
        LoadSnapshotOutcome::NonRegularArtifact { .. } => Ok(None),
        LoadSnapshotOutcome::ExpandedTooLarge { path, .. } => Err(anyhow!(
            "named session recovery is too large to open without mutating candidate artifacts: {}",
            path.display()
        )),
        LoadSnapshotOutcome::LoadedFromBackup(_) | LoadSnapshotOutcome::LoadedFromRecovery(_) => {
            Ok(None)
        }
    }
}

#[allow(dead_code)]
fn load_named_candidate_primary(
    session_path: &Path,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> Result<Option<LoadedSnapshot>> {
    let file = open_session_artifact_for_read(session_path, true)?;
    let metadata = file.metadata().with_context(|| {
        format!(
            "failed to inspect opened session file {}",
            session_path.display()
        )
    })?;
    if metadata.len() > options.max_file_size_bytes {
        return Err(anyhow!(
            "session file {} is {} bytes which exceeds the configured limit of {} bytes",
            session_path.display(),
            metadata.len(),
            options.max_file_size_bytes
        ));
    }
    load_snapshot_opened_with_expanded_limit(
        session_path,
        options,
        file,
        max_expanded_size,
        Some(options.max_file_size_bytes),
    )
}

fn load_snapshot_opened_with_expanded_limit(
    session_path: &Path,
    options: &SessionOptions,
    mut file: fs::File,
    max_expanded_size: u64,
    max_encoded_size: Option<u64>,
) -> Result<Option<LoadedSnapshot>> {
    let mut file_bytes = Vec::new();
    if let Some(max_encoded_size) = max_encoded_size {
        file.by_ref()
            .take(max_encoded_size.saturating_add(1))
            .read_to_end(&mut file_bytes)
            .context("failed to read session file")?;
        if file_bytes.len() as u64 > max_encoded_size {
            return Err(anyhow!(
                "session file {} is larger than configured limit of {} bytes",
                session_path.display(),
                max_encoded_size
            ));
        }
    } else {
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
    let named_primary = is_named_primary_path(session_path, options);
    let bytes = read_corrupt_session_bytes(session_path, named_primary)?;
    let primary_path = options.session_file_path();
    let backup_path = if session_path == primary_path.as_path() {
        options.backup_file_path()
    } else {
        options.corrupt_artifact_backup_file_path(session_path)
    };
    fs::write(&backup_path, &bytes)
        .with_context(|| format!("failed to write session backup {}", backup_path.display()))?;
    if named_primary {
        debug!(
            "Backed up corrupt named session primary {} to {}; leaving the selected primary in place",
            session_path.display(),
            backup_path.display()
        );
        return Ok(());
    }
    fs::remove_file(session_path).with_context(|| {
        format!(
            "failed to remove corrupt session {}",
            session_path.display()
        )
    })?;
    Ok(())
}

fn read_corrupt_session_bytes(session_path: &Path, no_follow: bool) -> Result<Vec<u8>> {
    if !no_follow {
        return fs::read(session_path)
            .with_context(|| format!("failed to read corrupt session {}", session_path.display()));
    }

    let mut file = open_session_artifact_for_read(session_path, true)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("failed to read corrupt session {}", session_path.display()))?;
    Ok(bytes)
}
