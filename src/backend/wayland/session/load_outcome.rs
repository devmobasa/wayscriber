//! What a launch-style load found, put on the canvas. A launch, an output's
//! session load and a return to the home session share these rules.

use std::path::PathBuf;

use anyhow::Result;
use log::{debug, warn};

use crate::input::InputState;
use crate::input::state::{Toast, ToastPriority};
use crate::session::{
    self as stored_session, LoadSnapshotOutcome, SessionOptions, SessionSnapshot,
};

/// A session left on disk unloaded because it expands beyond the safety cap.
#[derive(Debug)]
pub(in crate::backend::wayland) struct ExpandedTooLarge {
    pub(in crate::backend::wayland) path: PathBuf,
    pub(in crate::backend::wayland) max_expanded_size: u64,
}

/// Replaces every board with what loading `options` found, and gives the
/// notices a launch gives for a backup, recovery or unreadable session. A
/// session too large to restore is returned for the caller to protect and
/// report.
pub(in crate::backend::wayland) fn apply_load_outcome(
    input_state: &mut InputState,
    measurer: &crate::draw::TextMeasurer,
    outcome: LoadSnapshotOutcome,
    options: &SessionOptions,
    context: &str,
) -> Result<Option<ExpandedTooLarge>> {
    match outcome {
        LoadSnapshotOutcome::Loaded(snapshot) => {
            debug!(
                "Restoring session {} from {}",
                context,
                options.session_file_path().display()
            );
            replace_output_session_snapshot(input_state, measurer, Some(*snapshot), options)?;
        }
        LoadSnapshotOutcome::LoadedFromBackup(snapshot) => {
            warn!(
                "Restoring session {} from backup {} because the primary session had no board data",
                context,
                options.backup_file_path().display()
            );
            replace_output_session_snapshot(input_state, measurer, Some(*snapshot), options)?;
            input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Restored drawings from the session backup; the primary session had no board data."));
        }
        LoadSnapshotOutcome::LoadedFromRecovery(snapshot) => {
            debug!(
                "Restoring session {} from recovery artifact {}",
                context,
                options.recovery_file_path().display()
            );
            replace_output_session_snapshot(input_state, measurer, Some(*snapshot), options)?;
            input_state.push_toast(ToastPriority::Info, "output", Toast::warning("Restored session from recovery file; normal save previously exceeded the size limit."));
        }
        LoadSnapshotOutcome::Empty => {
            debug!(
                "No session data found for {} ({})",
                options.session_file_path().display(),
                context
            );
            replace_output_session_snapshot(input_state, measurer, None, options)?;
        }
        LoadSnapshotOutcome::EmptyAfterCorruption { backup_path } => {
            // An empty canvas here is indistinguishable from "no session
            // yet", so without this the user's drawings appear to have
            // vanished and only the log says the bytes were kept.
            warn!(
                "Session {} could not be read for {}; its bytes were preserved at {}",
                options.session_file_path().display(),
                context,
                backup_path.display()
            );
            replace_output_session_snapshot(input_state, measurer, None, options)?;
            input_state.push_toast(
                ToastPriority::Critical,
                "session.corrupt",
                Toast::error(format!(
                    "Previous session could not be read; a copy was saved to {}",
                    backup_path.display()
                ))
                .duration_ms(20_000),
            );
        }
        LoadSnapshotOutcome::NonRegularArtifact { path } => {
            debug!(
                "Skipping non-regular session artifact {} for {}",
                path.display(),
                context
            );
            replace_output_session_snapshot(input_state, measurer, None, options)?;
        }
        LoadSnapshotOutcome::ExpandedTooLarge {
            path,
            max_expanded_size,
        } => {
            replace_output_session_snapshot(input_state, measurer, None, options)?;
            return Ok(Some(ExpandedTooLarge {
                path,
                max_expanded_size,
            }));
        }
    }

    Ok(None)
}

/// Replaces every board with `snapshot`, or with empty boards without one.
pub(in crate::backend::wayland) fn replace_output_session_snapshot(
    input_state: &mut InputState,
    measurer: &crate::draw::TextMeasurer,
    snapshot: Option<SessionSnapshot>,
    options: &SessionOptions,
) -> Result<()> {
    let snapshot = snapshot.unwrap_or_else(|| SessionSnapshot {
        active_board_id: input_state.board_id().to_string(),
        boards: Vec::new(),
        tool_state: None,
    });
    stored_session::apply_snapshot_replacing_boards(input_state, measurer, snapshot, options)
}
