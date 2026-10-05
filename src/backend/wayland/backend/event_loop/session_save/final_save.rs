use std::path::PathBuf;

use super::*;

pub(super) fn persist_final_session_and_shutdown(
    state: &mut WaylandState,
) -> Result<(), anyhow::Error> {
    let save_result = persist_final_session(state);
    let worker_failed = !state.persistence.is_healthy();
    let shutdown_result = state.persistence.shutdown(state.session.target_epoch());
    if save_result.is_err() && worker_failed && state.persistence.is_stopped() {
        log::warn!(
            "Persistence worker failed before the final save; attempting joined event-thread fallback"
        );
        match persist_final_session_direct(state) {
            Ok(()) => {
                if let Err(shutdown) = shutdown_result {
                    log::warn!(
                        "Persistence worker shutdown failed before successful direct fallback: {shutdown:#}"
                    );
                }
                return Ok(());
            }
            Err(fallback) => {
                let original = save_result.expect_err("fallback requires failed worker save");
                // Context keeps the fallback's typed error for the exit notice.
                return Err(fallback.context(format!(
                    "worker final save failed: {original:#}; joined direct fallback also failed"
                )));
            }
        }
    }
    match (save_result, shutdown_result) {
        (Err(save), Err(shutdown)) => Err(save.context(format!(
            "final session save failed; persistence worker shutdown also failed: {shutdown:#}"
        ))),
        (Err(save), Ok(())) => Err(save),
        (Ok(()), Err(shutdown)) => Err(shutdown),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn persist_final_session_direct(state: &mut WaylandState) -> Result<(), anyhow::Error> {
    observe_input_dirty(state, Instant::now());
    let Some(options) = state.session_options().cloned() else {
        return Ok(());
    };
    match state
        .session
        .validate_source_write(state.input_state.is_session_dirty())
    {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(refusal) => return save_refused_drawings(state, &options, refusal, SideWrite::Direct),
    }
    if should_skip_disabled_final_save(&options) {
        return Ok(());
    }
    if should_skip_protected_session_save(state, &options) {
        return Ok(());
    }
    let snapshot = state
        .input_state
        .snapshot_for_persistence_with(state.render.text_measurer(), &options);
    let has_board_data = snapshot
        .as_ref()
        .is_some_and(session::SessionSnapshot::has_board_data);
    if runtime_session::should_skip_unloaded_contentless_save(
        state.session.has_loaded_board_data(),
        state.session.is_dirty(),
        state.input_state.is_session_dirty(),
        has_board_data,
        runtime_session::has_session_artifact(&options),
    ) {
        return Ok(());
    }
    let snapshot = snapshot_or_empty(state, &options, snapshot)?;
    let snapshot_board_data = snapshot.has_board_data();
    let report = session::save_snapshot_with_report_and_clear_boundary(
        &snapshot,
        &options,
        state.session.has_loaded_board_data(),
    )?;
    let Some(report) = report else {
        return Err(anyhow::anyhow!(
            "joined direct fallback produced no committed session write"
        ));
    };
    let committed_board_data =
        !matches!(report.outcome, session::SaveSnapshotOutcome::ClearedEmpty)
            && snapshot_board_data;
    log_session_save_result(SessionSaveReason::Shutdown, Some(&report), Duration::ZERO);
    state
        .session
        .mark_saved(Instant::now(), committed_board_data);
    Ok(())
}

fn persist_final_session(state: &mut WaylandState) -> Result<(), anyhow::Error> {
    let barrier_result = persistence_barrier(state);
    if let Some(err) = final_save_barrier_policy(barrier_result, state.persistence.is_healthy())? {
        log::warn!(
            "Autosave failed while preparing the final session save; retrying the current live state with the normal save strategy: {err:#}"
        );
    }
    if let Some(path) = state.unloaded_remembered_session()
        && !state.session.is_dirty()
        && !state.input_state.is_session_dirty()
        && session::validate_named_session_file_for_open(&path).is_err()
        && let Some(options) = state.session_options().cloned()
    {
        let identity = state
            .surface
            .current_output()
            .as_ref()
            .and_then(|output| state.output_identity_for(output));
        state.load_configured_session_for_options(
            options,
            identity.as_deref(),
            "shutdown remembered fallback",
        )?;
    }
    let Some(options) = state.session_options().cloned() else {
        return Ok(());
    };
    match state
        .session
        .validate_source_write(state.input_state.is_session_dirty())
    {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(refusal) => return save_refused_drawings(state, &options, refusal, SideWrite::Worker),
    }
    if should_skip_disabled_final_save(&options) {
        return Ok(());
    }

    if should_skip_protected_session_save(state, &options) {
        return Ok(());
    }

    let started = Instant::now();
    log::info!(
        "Starting {} session persistence to {}",
        SessionSaveReason::Shutdown.label(),
        options.session_file_path().display()
    );
    let snapshot_started = Instant::now();
    let snapshot = state
        .input_state
        .snapshot_for_persistence_with(state.render.text_measurer(), &options);
    log_snapshot_capture(
        SessionSaveReason::Shutdown,
        &options,
        snapshot.as_ref(),
        snapshot_started.elapsed(),
    );
    if should_skip_unloaded_contentless_save(state, &options, snapshot.as_ref())? {
        return Ok(());
    }
    let snapshot = snapshot_or_empty(state, &options, snapshot)?;
    let outcome = run_persistence_operation(
        state,
        PersistenceOperation::Save {
            snapshot,
            options,
            strategy: SaveStrategy::Normal,
            contentless_clear_boundary: state.session.has_loaded_board_data(),
        },
    )?;
    let PersistenceOutcome::Save(save) = outcome else {
        return Err(anyhow::anyhow!("unexpected final-save worker outcome"));
    };
    if !save.committed() {
        return Err(anyhow::anyhow!(
            "final session save produced no committed write"
        ));
    }
    log_session_save_result(
        SessionSaveReason::Shutdown,
        save.report.as_ref(),
        started.elapsed(),
    );
    notify_session_save_report(state, save.report.as_ref());
    state
        .session
        .mark_saved(Instant::now(), save.committed_board_data);
    Ok(())
}

/// Drawings that the write policy keeps out of their session file, because
/// that session never loaded or its unreadable bytes could not be preserved,
/// were saved beside it at exit instead of closing with the overlay.
#[derive(Debug)]
pub(super) struct RefusedDrawingsSaved {
    pub(super) session: PathBuf,
    pub(super) saved_to: PathBuf,
    /// Set when the drawings exceed the size limit and were kept in the new
    /// session's recovery file, which opening that session restores.
    pub(super) recovery: Option<PathBuf>,
}

impl std::fmt::Display for RefusedDrawingsSaved {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "drawings that could not be written to {} were saved to {}",
            self.session.display(),
            self.saved_to.display()
        )?;
        if let Some(recovery) = &self.recovery {
            write!(formatter, " (recovery file {})", recovery.display())?;
        }
        Ok(())
    }
}

impl std::error::Error for RefusedDrawingsSaved {}

#[derive(Clone, Copy)]
enum SideWrite {
    Worker,
    Direct,
}

/// Saves drawings the session file refused into a new named session beside
/// it, so closing the overlay does not discard them. Only tool settings, with
/// no drawings, are not worth a file.
fn save_refused_drawings(
    state: &mut WaylandState,
    options: &session::SessionOptions,
    refusal: anyhow::Error,
    write: SideWrite,
) -> Result<(), anyhow::Error> {
    let side = refused_drawings_options(options);
    let snapshot = state
        .input_state
        .snapshot_for_persistence_with(state.render.text_measurer(), &side)
        .filter(session::SessionSnapshot::has_board_data);
    let Some(snapshot) = snapshot else {
        log::info!("Final save skipped: {refusal:#}; no drawings to keep");
        return Ok(());
    };

    Err(match write_side_session(state, &side, snapshot, write) {
        Ok(recovery) => RefusedDrawingsSaved {
            session: options.session_file_path(),
            saved_to: side.session_file_path(),
            recovery,
        }
        .into(),
        Err(error) => anyhow::anyhow!(
            "drawings that could not be written to {} were lost: saving them to {} also failed: {error:#}",
            options.session_file_path().display(),
            side.session_file_path().display()
        ),
    })
}

/// Writes `snapshot` as a new session at `side`, returning the recovery file
/// when the drawings exceed the size limit. Like an oversized normal save,
/// those drawings go to the recovery file beside a primary that holds only
/// tool settings, and opening the session restores them.
fn write_side_session(
    state: &mut WaylandState,
    side: &session::SessionOptions,
    snapshot: session::SessionSnapshot,
    write: SideWrite,
) -> Result<Option<PathBuf>, anyhow::Error> {
    let error = match save_side_session_as(state, side, snapshot.clone(), write) {
        Ok(()) => return Ok(None),
        Err(error)
            if error
                .downcast_ref::<session::SavePayloadTooLarge>()
                .is_some() =>
        {
            error
        }
        Err(error) => return Err(error),
    };
    log::warn!(
        "Refused drawings exceed the session size limit; keeping them in recovery: {error:#}"
    );

    let settings_only = session::SessionSnapshot {
        active_board_id: snapshot.active_board_id.clone(),
        boards: Vec::new(),
        tool_state: snapshot.tool_state.clone(),
    };
    save_side_session_as(state, side, settings_only, write)?;
    let saved = match write {
        SideWrite::Worker => run_persistence_operation(
            state,
            PersistenceOperation::Save {
                snapshot,
                options: side.clone(),
                strategy: SaveStrategy::Normal,
                contentless_clear_boundary: false,
            },
        )
        .map(drop),
        SideWrite::Direct => {
            session::save_snapshot_with_report_and_clear_boundary(&snapshot, side, false).map(drop)
        }
    };

    match saved {
        Ok(()) => Ok(None),
        Err(error) => error
            .downcast_ref::<session::SavePayloadTooLarge>()
            .and_then(|limit| limit.recovery_path.clone())
            .map(Some)
            .ok_or(error),
    }
}

/// A Save As that never replaces an existing file.
fn save_side_session_as(
    state: &mut WaylandState,
    side: &session::SessionOptions,
    snapshot: session::SessionSnapshot,
    write: SideWrite,
) -> Result<(), anyhow::Error> {
    match write {
        SideWrite::Worker => run_persistence_operation(
            state,
            PersistenceOperation::SaveAs {
                snapshot,
                options: side.clone(),
                overwrite: session::SaveAsOverwrite::Deny,
            },
        )
        .map(drop),
        SideWrite::Direct => {
            session::save_snapshot_as_with_report(&snapshot, side, session::SaveAsOverwrite::Deny)
                .map(|_| session::catalog::record_named_session_saved(side))
        }
    }
}

/// `<name>.unsaved-<milliseconds>.wayscriber-session` beside the session file.
fn refused_drawings_options(options: &session::SessionOptions) -> session::SessionOptions {
    let path = options.session_file_path();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or_default();
    let mut name = path
        .file_stem()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_else(|| "session".into());
    name.push(format!(".unsaved-{millis}.wayscriber-session"));
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));

    let mut side = options.clone();
    side.set_named_file_target(parent.join(name));
    side.force_resume_persistence();
    side
}
