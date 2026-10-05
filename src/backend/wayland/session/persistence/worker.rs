//! Background disk operations and completion-before-wake publication.
use std::path::Path;

use super::*;
use crate::session::SaveSnapshotOutcome;

pub(super) fn worker_main(
    request_rx: Receiver<PersistenceRequest>,
    completion_tx: SyncSender<PersistenceCompletion>,
    wake: RuntimeWakeHandle,
) {
    let publisher = PersistenceCompletionPublisher::new(completion_tx, wake);
    while let Ok(request) = request_rx.recv() {
        let PersistenceRequest {
            id,
            queued_at,
            operation,
        } = request;
        let queue_wait = queued_at.elapsed();
        let label = operation.label();
        let shutdown = matches!(operation, PersistenceOperation::Shutdown);
        let started = Instant::now();
        log::debug!("Persistence worker starting {label} request {id:?}");
        let result = execute(operation);
        let execution_time = started.elapsed();
        let worker_thread_id = thread::current().id();
        let finished_at = Instant::now();
        if !publisher.publish(PersistenceCompletion {
            id,
            result,
            queue_wait,
            execution_time,
            worker_thread_id,
            finished_at,
        }) {
            break;
        }
        if shutdown {
            break;
        }
    }
}

struct PersistenceCompletionPublisher {
    completion_tx: Option<SyncSender<PersistenceCompletion>>,
    wake: RuntimeWakeHandle,
}

impl PersistenceCompletionPublisher {
    fn new(completion_tx: SyncSender<PersistenceCompletion>, wake: RuntimeWakeHandle) -> Self {
        Self {
            completion_tx: Some(completion_tx),
            wake,
        }
    }

    fn publish(&self, completion: PersistenceCompletion) -> bool {
        let Some(completion_tx) = self.completion_tx.as_ref() else {
            return false;
        };
        if completion_tx.send(completion).is_err() {
            return false;
        }
        if let Err(err) = self.wake.wake() {
            log::error!("Failed to wake runtime after persistence completion: {err}");
            return false;
        }
        true
    }
}

impl Drop for PersistenceCompletionPublisher {
    fn drop(&mut self) {
        // Close the completion channel before waking. The event loop can therefore
        // observe disconnect immediately even when the worker unwinds without a
        // completion packet.
        self.completion_tx.take();
        if let Err(err) = self.wake.wake() {
            log::error!("Failed to wake runtime after persistence worker exit: {err}");
        }
    }
}

pub(super) fn execute(operation: PersistenceOperation) -> Result<PersistenceOutcome> {
    match operation {
        PersistenceOperation::Save {
            snapshot,
            options,
            strategy,
            contentless_clear_boundary,
        } => {
            log_snapshot_summary(&snapshot, &options, strategy);
            let snapshot_board_data = snapshot.has_board_data();
            let report = match strategy {
                SaveStrategy::Autosave => {
                    session::save_snapshot_autosave_with_report_and_clear_boundary(
                        &snapshot,
                        &options,
                        contentless_clear_boundary,
                    )?
                }
                SaveStrategy::Normal => session::save_snapshot_with_report_and_clear_boundary(
                    &snapshot,
                    &options,
                    contentless_clear_boundary,
                )?,
            };
            let committed_board_data = report.as_ref().is_some_and(|report| {
                !matches!(report.outcome, SaveSnapshotOutcome::ClearedEmpty) && snapshot_board_data
            });
            Ok(PersistenceOutcome::Save(SaveCompletion {
                report,
                committed_board_data,
            }))
        }
        PersistenceOperation::SaveAs {
            snapshot,
            options,
            overwrite,
        } => {
            let snapshot_board_data = snapshot.has_board_data();
            let report = session::save_snapshot_as_with_report(&snapshot, &options, overwrite)?;
            let committed_board_data =
                !matches!(report.outcome, SaveSnapshotOutcome::ClearedEmpty) && snapshot_board_data;
            session::catalog::record_named_session_saved(&options);
            Ok(PersistenceOutcome::SaveAs {
                report,
                committed_board_data,
            })
        }
        PersistenceOperation::LoadConfigured { options } => Ok(PersistenceOutcome::Load(
            session::load_snapshot_with_outcome(&options)?,
        )),
        PersistenceOperation::LoadNamedCandidate { options } => Ok(PersistenceOutcome::Load(
            session::load_named_session_candidate(&options)?,
        )),
        PersistenceOperation::LoadHome { options } => load_home(&options),
        PersistenceOperation::LoadRemembered { options } => load_remembered(&options),
        PersistenceOperation::CheckRemembered { path } => {
            Ok(match session::validate_named_session_file_for_open(&path) {
                Ok(()) => PersistenceOutcome::Unit,
                Err(error) => PersistenceOutcome::RememberedUnavailable(error),
            })
        }
        PersistenceOperation::Inspect { options } => Ok(PersistenceOutcome::Inspection(
            session::inspect_session(&options)?,
        )),
        PersistenceOperation::SaveAsOverwritePreflight {
            current_path,
            options,
        } => {
            // Validation must run before identity matching: identity canonicalization follows
            // symlinks, while named foreground targets must reject them.
            let target_path = options.session_file_path();
            session::validate_named_session_file_for_foreground(&target_path)?;
            let (same_target, overwrite_required) =
                save_as_preflight_after_validation(&current_path, &target_path, || {
                    session::save_snapshot_as_requires_overwrite(&options)
                })?;
            Ok(PersistenceOutcome::SaveAsPreflight {
                same_target,
                overwrite_required,
            })
        }
        PersistenceOperation::ValidateNamedOpen { path } => {
            session::validate_named_session_file_for_open(&path)?;
            Ok(PersistenceOutcome::Unit)
        }
        PersistenceOperation::ClearToolState { options } => Ok(
            PersistenceOutcome::ToolStateCleared(session::clear_tool_state(&options)?),
        ),
        PersistenceOperation::HasArtifacts { options } => Ok(PersistenceOutcome::HasArtifacts(
            super::super::has_session_artifact(&options),
        )),
        PersistenceOperation::RecordNamedOpened { options } => {
            session::catalog::try_record_named_session_opened(&options)?;
            Ok(PersistenceOutcome::Unit)
        }
        PersistenceOperation::ForgetNamedSessionByPath { path } => Ok(
            PersistenceOutcome::CatalogForgotten(session::catalog::forget_session_by_path(&path)?),
        ),
        #[cfg(test)]
        PersistenceOperation::PanicForTest => {
            panic!("intentional persistence worker panic for disconnect testing")
        }
        PersistenceOperation::Shutdown => Ok(PersistenceOutcome::Unit),
    }
}

/// Loads home as a launch would, but only from a session file that later saves
/// can replace, checked before and after the load: a named home passes the
/// startup checks of a session file, and a configured home's file must be a
/// regular file or not exist yet. A launch would follow a symlink or start
/// from a recovery copy beside a directory, then fail every save.
fn load_home(options: &SessionOptions) -> Result<PersistenceOutcome> {
    let path = options.session_file_path();
    let check = || {
        if options.is_named_file() {
            session::validate_named_session_file_for_foreground(&path)
        } else {
            require_replaceable_session_file(&path)
        }
    };

    check()?;
    let outcome = session::load_snapshot_with_outcome(options)?;
    check()?;
    Ok(PersistenceOutcome::Load(outcome))
}

/// A session file that a save can replace: a regular file, or none yet.
fn require_replaceable_session_file(path: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect session file {}", path.display()));
        }
    };
    let kind = if metadata.is_file() {
        return Ok(());
    } else if metadata.file_type().is_symlink() {
        "a symlink"
    } else if metadata.is_dir() {
        "a directory"
    } else {
        "not a regular file"
    };
    Err(anyhow!("session file {} is {kind}", path.display()))
}

/// Loads a remembered session with the startup rules of a session file, but
/// only while its primary still exists. Its own newer recovery and marked
/// backup use normal startup precedence. Sidecars cannot continue a file that
/// was moved, deleted, or became non-regular before or during the load.
fn load_remembered(options: &SessionOptions) -> Result<PersistenceOutcome> {
    let path = options.session_file_path();
    if let Err(error) = session::validate_named_session_file_for_open(&path) {
        return Ok(PersistenceOutcome::RememberedUnavailable(error));
    }
    let outcome = match session::load_snapshot_with_outcome(options) {
        Ok(outcome) => outcome,
        Err(error) => {
            if error
                .downcast_ref::<session::CorruptArtifactPreservationFailed>()
                .is_some()
            {
                return Ok(PersistenceOutcome::RememberedUnavailable(error));
            }
            return match session::validate_named_session_file_for_open(&path) {
                Ok(()) => Err(error),
                Err(unavailable) => Ok(PersistenceOutcome::RememberedUnavailable(unavailable)),
            };
        }
    };
    if let LoadSnapshotOutcome::EmptyAfterCorruption { corrupt_copy } = outcome {
        return Ok(PersistenceOutcome::RememberedUnavailable(anyhow!(
            "remembered session {} is corrupt; its bytes are preserved at {}",
            path.display(),
            corrupt_copy.display()
        )));
    }

    // A corruption restore can leave the primary absent when quarantine moved
    // it aside and recovered bytes exceed the size limit for primary repair.
    Ok(match session::validate_named_session_file_for_open(&path) {
        Ok(()) => PersistenceOutcome::Load(outcome),
        Err(error) => PersistenceOutcome::RememberedUnavailable(error),
    })
}

pub(super) fn save_as_preflight_after_validation(
    current_path: &Path,
    target_path: &Path,
    discover_overwrite: impl FnOnce() -> Result<bool>,
) -> Result<(bool, bool)> {
    let same_target = session::catalog::session_paths_match(current_path, target_path);
    if same_target {
        return Ok((true, false));
    }
    Ok((false, discover_overwrite()?))
}

fn log_snapshot_summary(
    snapshot: &SessionSnapshot,
    options: &SessionOptions,
    strategy: SaveStrategy,
) {
    let mut boards = 0usize;
    let mut pages = 0usize;
    let mut shapes = 0usize;
    let mut undo_entries = 0usize;
    let mut redo_entries = 0usize;
    let mut visible_image_shapes = 0usize;
    let mut visible_image_bytes = 0usize;
    let mut max_history_depth = 0usize;
    for board in &snapshot.boards {
        boards += 1;
        pages += board.pages.pages.len();
        for frame in &board.pages.pages {
            let undo = frame.undo_stack_len();
            let redo = frame.redo_stack_len();
            shapes += frame.shapes.len();
            undo_entries += undo;
            redo_entries += redo;
            max_history_depth = max_history_depth.max(undo.max(redo));
            for drawn in &frame.shapes {
                if let crate::draw::Shape::Image { data, .. } = &drawn.shape {
                    visible_image_shapes += 1;
                    visible_image_bytes = visible_image_bytes.saturating_add(data.bytes.len());
                }
            }
        }
    }
    log::info!(
        "Persistence worker snapshot diagnostics for {} ({strategy:?}): boards={boards}, pages={pages}, shapes={shapes}, undo_entries={undo_entries}, redo_entries={redo_entries}, max_history_depth={max_history_depth}, visible_images={visible_image_shapes} ({visible_image_bytes} bytes), tool_state={}",
        options.session_file_path().display(),
        snapshot.tool_state.is_some()
    );
}
