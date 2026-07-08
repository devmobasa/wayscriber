use super::*;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeOpenSessionReport {
    pub previous_path: PathBuf,
    pub opened_path: PathBuf,
    pub saved_current: bool,
    pub loaded_board_data: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeSaveAsSessionReport {
    pub previous_path: PathBuf,
    pub saved_path: PathBuf,
    pub switched_target: bool,
    pub saved: bool,
    pub saved_board_data: bool,
    pub outcome: Option<stored_session::SaveSnapshotOutcome>,
    pub written_size: Option<usize>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeClearSessionReport {
    pub cleared_path: PathBuf,
    pub persisted: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RuntimeClearToolStateReport {
    pub session_path: Option<PathBuf>,
    pub outcome: Option<stored_session::ClearToolStateOutcome>,
}

#[allow(dead_code)]
pub(in crate::backend::wayland) fn open_named_session_runtime(
    input_state: &mut InputState,
    session_state: &mut SessionState,
    target_path: &Path,
    now: Instant,
) -> Result<RuntimeOpenSessionReport> {
    let current_options = session_state
        .options()
        .cloned()
        .ok_or_else(|| anyhow!("cannot open session without active session options"))?;
    let previous_path = current_options.session_file_path();

    let saved_current = save_current_session_before_runtime_open(
        input_state,
        session_state,
        &current_options,
        now,
    )?;

    let mut candidate_options = current_options;
    candidate_options.set_named_file_target(target_path.to_path_buf());
    candidate_options.force_resume_persistence();

    let outcome = stored_session::load_named_session_candidate(&candidate_options)?;
    let candidate_snapshot = match outcome {
        LoadSnapshotOutcome::Loaded(snapshot)
        | LoadSnapshotOutcome::LoadedFromBackup(snapshot)
        | LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => *snapshot,
        LoadSnapshotOutcome::Empty => {
            return Err(anyhow!(
                "named session file contains no usable session data: {}",
                candidate_options.session_file_path().display()
            ));
        }
        LoadSnapshotOutcome::NonRegularArtifact { path } => {
            return Err(anyhow!(
                "named session file is not a regular file: {}",
                path.display()
            ));
        }
        LoadSnapshotOutcome::ExpandedTooLarge {
            path,
            max_expanded_size,
        } => {
            return Err(anyhow!(
                "named session file expands beyond the {} byte safety limit: {}",
                max_expanded_size,
                path.display()
            ));
        }
    };

    let loaded_board_data = candidate_snapshot.has_board_data();
    stored_session::apply_snapshot_replacing_boards(
        input_state,
        candidate_snapshot,
        &candidate_options,
    )?;
    input_state.set_session_preflight_options(Some(candidate_options.clone()));
    input_state.clear_session_dirty();
    let opened_path = candidate_options.session_file_path();
    session_state.commit_runtime_open(candidate_options.clone(), loaded_board_data);
    stored_session::catalog::record_named_session_opened(&candidate_options);

    Ok(RuntimeOpenSessionReport {
        previous_path,
        opened_path,
        saved_current,
        loaded_board_data,
    })
}

#[allow(dead_code)]
pub(in crate::backend::wayland) fn save_named_session_as_runtime(
    input_state: &mut InputState,
    session_state: &mut SessionState,
    target_path: &Path,
    overwrite: stored_session::SaveAsOverwrite,
    now: Instant,
) -> Result<RuntimeSaveAsSessionReport> {
    let current_options = session_state
        .options()
        .cloned()
        .ok_or_else(|| anyhow!("cannot save session as without active session options"))?;
    let previous_path = current_options.session_file_path();

    stored_session::validate_named_session_file_for_foreground(target_path)?;
    if stored_session::catalog::session_paths_match(&previous_path, target_path) {
        let saved = save_current_session_before_runtime_open(
            input_state,
            session_state,
            &current_options,
            now,
        )?;
        return Ok(RuntimeSaveAsSessionReport {
            previous_path: previous_path.clone(),
            saved_path: previous_path,
            switched_target: false,
            saved,
            saved_board_data: session_state.has_loaded_board_data(),
            outcome: None,
            written_size: None,
        });
    }

    let mut target_options = current_options;
    target_options.set_named_file_target(target_path.to_path_buf());
    target_options.force_resume_persistence();

    let snapshot = input_state
        .with_active_interaction_canceled_for_capture(|input_state| {
            stored_session::snapshot_from_input(input_state, &target_options)
        })
        .ok_or_else(|| anyhow!("Save Session As has no session data to write"))?;
    let saved_board_data = snapshot.has_board_data();
    let save_report =
        stored_session::save_snapshot_as_with_report(&snapshot, &target_options, overwrite)?;

    input_state.set_session_preflight_options(Some(target_options.clone()));
    let _ = input_state.take_session_dirty();
    input_state.clear_session_dirty();
    let saved_path = target_options.session_file_path();
    session_state.commit_runtime_save_as(target_options.clone(), now, saved_board_data);
    stored_session::catalog::record_named_session_saved(&target_options);

    Ok(RuntimeSaveAsSessionReport {
        previous_path,
        saved_path,
        switched_target: true,
        saved: true,
        saved_board_data,
        outcome: Some(save_report.outcome),
        written_size: Some(save_report.written_size),
    })
}

#[allow(dead_code)]
pub(in crate::backend::wayland) fn save_named_session_as_requires_overwrite(
    session_state: &SessionState,
    target_path: &Path,
) -> Result<bool> {
    let current_options = session_state
        .options()
        .cloned()
        .ok_or_else(|| anyhow!("cannot save session as without active session options"))?;
    let previous_path = current_options.session_file_path();

    stored_session::validate_named_session_file_for_foreground(target_path)?;
    if stored_session::catalog::session_paths_match(&previous_path, target_path) {
        return Ok(false);
    }

    let mut target_options = current_options;
    target_options.set_named_file_target(target_path.to_path_buf());
    target_options.force_resume_persistence();
    stored_session::save_snapshot_as_requires_overwrite(&target_options)
}

#[allow(dead_code)]
pub(in crate::backend::wayland) fn clear_current_session_runtime(
    input_state: &mut InputState,
    session_state: &mut SessionState,
    now: Instant,
) -> Result<RuntimeClearSessionReport> {
    let options = session_state
        .options()
        .cloned()
        .ok_or_else(|| anyhow!("cannot clear session without active session options"))?;
    let cleared_path = options.session_file_path();
    let empty_snapshot = stored_session::SessionSnapshot {
        active_board_id: input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    };
    let report = stored_session::save_snapshot_with_report_and_clear_boundary(
        &empty_snapshot,
        &options,
        true,
    )?;
    if report.is_none() {
        return Err(anyhow!(
            "current session clear did not write a durable clear boundary"
        ));
    }

    stored_session::apply_snapshot_replacing_boards(input_state, empty_snapshot, &options)?;
    input_state.set_session_preflight_options(Some(options));
    let _ = input_state.take_session_dirty();
    input_state.clear_session_dirty();
    session_state.commit_runtime_clear(now);

    Ok(RuntimeClearSessionReport {
        cleared_path,
        persisted: true,
    })
}

#[allow(dead_code)]
pub(in crate::backend::wayland) fn clear_saved_tool_state_runtime(
    input_state: &mut InputState,
    session_state: &mut SessionState,
    default_tool_state: stored_session::ToolStateSnapshot,
    now: Instant,
) -> Result<RuntimeClearToolStateReport> {
    let (session_path, outcome) = if let Some(options) = session_state.options().cloned() {
        let session_path = options.session_file_path();
        let outcome = stored_session::clear_tool_state(&options)?;
        (Some(session_path), Some(outcome))
    } else {
        (None, None)
    };

    stored_session::apply_tool_state_snapshot(input_state, default_tool_state);
    input_state.mark_session_dirty();
    session_state.record_input_dirty(now, true);

    Ok(RuntimeClearToolStateReport {
        session_path,
        outcome,
    })
}

fn save_current_session_before_runtime_open(
    input_state: &mut InputState,
    session_state: &mut SessionState,
    options: &SessionOptions,
    now: Instant,
) -> Result<bool> {
    if !input_state.is_session_dirty() && !session_state.is_dirty() {
        return Ok(false);
    }

    let snapshot = input_state.with_active_interaction_canceled_for_capture(|input_state| {
        stored_session::snapshot_from_input(input_state, options)
    });
    if should_skip_unloaded_contentless_save(
        session_state.has_loaded_board_data(),
        session_state.is_dirty(),
        input_state.is_session_dirty(),
        snapshot
            .as_ref()
            .is_some_and(stored_session::SessionSnapshot::has_board_data),
        has_session_artifact(options),
    ) {
        return Ok(false);
    }

    let saved_board_data = snapshot
        .as_ref()
        .is_some_and(stored_session::SessionSnapshot::has_board_data);
    let report = if let Some(snapshot) = snapshot {
        stored_session::save_snapshot_with_report_and_clear_boundary(
            &snapshot,
            options,
            session_state.has_loaded_board_data(),
        )?
    } else if persistence_enabled(options) {
        let empty_snapshot = stored_session::SessionSnapshot {
            active_board_id: input_state.board_id().to_string(),
            boards: Vec::new(),
            tool_state: None,
        };
        stored_session::save_snapshot_with_report_and_clear_boundary(
            &empty_snapshot,
            options,
            session_state.has_loaded_board_data(),
        )?
    } else {
        return Err(anyhow!(
            "current session has unsaved changes but persistence is disabled"
        ));
    };

    if report.is_none() {
        return Err(anyhow!(
            "current session had unsaved changes but no session file was written"
        ));
    }

    let _ = input_state.take_session_dirty();
    session_state.mark_saved(now, saved_board_data);
    Ok(true)
}

fn persistence_enabled(options: &SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

pub(in crate::backend::wayland) fn has_session_artifact(options: &SessionOptions) -> bool {
    options.session_file_path().exists()
        || options.backup_file_path().exists()
        || options.backup_recovery_marker_file_path().exists()
        || options.clear_marker_file_path().exists()
        || options.recovery_recoverable_marker_file_path().exists()
        || has_recovery_artifact(options)
}

fn has_recovery_artifact(options: &SessionOptions) -> bool {
    let recovery_path = options.recovery_file_path();
    if recovery_path.exists() {
        return true;
    }
    let Some(parent) = recovery_path.parent() else {
        return false;
    };
    let Some(recovery_name) = recovery_path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let preserved_prefix = format!("{recovery_name}.");
    let Ok(entries) = std::fs::read_dir(parent) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        path.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&preserved_prefix))
    })
}

pub(in crate::backend::wayland) fn should_skip_unloaded_contentless_save(
    loaded_board_data: bool,
    session_dirty: bool,
    input_dirty: bool,
    has_board_data: bool,
    session_artifact_exists: bool,
) -> bool {
    !has_board_data
        && !loaded_board_data
        && !session_dirty
        && !input_dirty
        && session_artifact_exists
}
