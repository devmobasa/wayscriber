use anyhow::{Result, anyhow};
use std::collections::BTreeSet;
use std::fs;

use super::types::ClearToolStateOutcome;
use crate::session::options::SessionOptions;
use crate::session::snapshot::{self, LoadSnapshotOutcome};

/// Remove only the saved tool state from the session snapshot.
pub fn clear_tool_state(options: &SessionOptions) -> Result<ClearToolStateOutcome> {
    if options.is_named_file() {
        crate::session::validate_named_session_file_for_clear(&options.session_file_path())?;
    }

    let targets = tool_state_edit_targets(options);
    let mut found_session = false;
    let mut cleared = false;
    let mut preserved_board_data = false;

    for edit_options in targets {
        match clear_tool_state_for_target(&edit_options)? {
            ClearToolStateOutcome::NoSession => {}
            ClearToolStateOutcome::NoToolState => found_session = true,
            ClearToolStateOutcome::Cleared {
                preserved_board_data: target_preserved_board_data,
            } => {
                found_session = true;
                cleared = true;
                preserved_board_data |= target_preserved_board_data;
            }
        }
    }

    if cleared {
        Ok(ClearToolStateOutcome::Cleared {
            preserved_board_data,
        })
    } else if found_session {
        Ok(ClearToolStateOutcome::NoToolState)
    } else {
        Ok(ClearToolStateOutcome::NoSession)
    }
}

fn clear_tool_state_for_target(edit_options: &SessionOptions) -> Result<ClearToolStateOutcome> {
    let mut snapshot = match snapshot::load_snapshot_for_offline_edit(edit_options)? {
        LoadSnapshotOutcome::Loaded(snapshot)
        | LoadSnapshotOutcome::LoadedFromBackup(snapshot)
        | LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => *snapshot,
        LoadSnapshotOutcome::Empty => return Ok(ClearToolStateOutcome::NoSession),
        LoadSnapshotOutcome::NonRegularArtifact { path } => {
            return Err(anyhow!(
                "cannot clear saved tool state because session artifact is not a regular file: {}",
                path.display()
            ));
        }
        LoadSnapshotOutcome::ExpandedTooLarge {
            path,
            max_expanded_size,
        } => {
            return Err(anyhow!(
                "cannot clear saved tool state because session file {} expands beyond the safety limit of {} bytes",
                path.display(),
                max_expanded_size
            ));
        }
    };

    if snapshot.tool_state.is_none() {
        return Ok(ClearToolStateOutcome::NoToolState);
    }

    let preserved_board_data = snapshot.has_board_data();
    snapshot.tool_state = None;
    snapshot::save_snapshot(&snapshot, edit_options)?;

    Ok(ClearToolStateOutcome::Cleared {
        preserved_board_data,
    })
}

fn tool_state_edit_targets(options: &SessionOptions) -> Vec<SessionOptions> {
    let mut identities = BTreeSet::new();
    if !options.is_named_file()
        && let Some(identity) = options.output_identity()
    {
        identities.insert(Some(identity.to_string()));
    } else {
        identities.insert(None);
    }

    if !options.is_named_file() && options.per_output && options.output_identity().is_none() {
        let prefix = options.file_prefix();
        if let Ok(entries) = fs::read_dir(&options.base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                if let Some(identity) = per_output_identity_from_snapshot_artifact(name, &prefix) {
                    identities.insert(identity);
                }
            }
        }
    }

    identities
        .into_iter()
        .map(|identity| {
            let mut target = options_for_tool_state_edit(options);
            if !target.is_named_file() {
                target.set_output_identity(identity.as_deref());
            }
            target
        })
        .collect()
}

fn per_output_identity_from_snapshot_artifact(name: &str, prefix: &str) -> Option<Option<String>> {
    let primary = name
        .strip_suffix(".bak")
        .or_else(|| name.strip_suffix(".recovery"))
        .unwrap_or(name);
    let rest = primary.strip_prefix(prefix)?;
    if rest == ".json" {
        return Some(None);
    }
    let identity = rest.strip_prefix('-')?.strip_suffix(".json")?;
    (!identity.is_empty()).then(|| Some(identity.to_string()))
}

fn options_for_tool_state_edit(options: &SessionOptions) -> SessionOptions {
    let mut options = options.clone();
    options.force_resume_persistence();
    options.max_shapes_per_frame = usize::MAX;
    options.max_persisted_undo_depth = None;
    options
}
