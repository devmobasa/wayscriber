//! Configurator-facing session maintenance APIs.

pub mod artifacts;
pub mod catalog;
mod lock;
mod options;
mod primary;
mod storage;

#[allow(unused_imports)]
pub use artifacts::{
    NamedSessionClearOutcome, NamedSessionDuplicateOutcome, NamedSessionMoveOutcome,
    NamedSessionMovedArtifact, SessionArtifactPaths, clear_named_session_non_lock_artifacts,
    duplicate_named_session_primary, move_named_session_non_lock_artifacts,
    named_session_artifact_paths, named_session_non_lock_artifact_paths,
    rollback_named_session_non_lock_artifacts_move,
};
pub use lock::try_lock_exclusive;
#[allow(unused_imports)]
pub(crate) use options::append_path_suffix;
#[allow(unused_imports)]
pub use options::{
    CompressionMode, DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES, MissingNamedSessionFile,
    MissingNamedSessionParent, SessionOptions, SessionTarget, normalize_named_session_file_arg,
    options_from_config, options_from_config_for_named_file, validate_named_session_file_for_clear,
    validate_named_session_file_for_foreground, validate_named_session_file_for_info,
    validate_named_session_file_for_open,
};
#[allow(unused_imports)]
pub use storage::{ClearOutcome, ClearToolStateOutcome, clear_session, clear_tool_state};

#[cfg(test)]
pub use snapshot_test_support::{
    BoardPagesSnapshot, BoardSnapshot, SessionSnapshot, load_snapshot, save_snapshot,
};

#[cfg(test)]
mod snapshot_test_support {
    use super::SessionOptions;
    use crate::draw::Frame;
    use anyhow::{Context, Result};
    use serde::{Deserialize, Serialize};
    use std::fs;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SessionSnapshot {
        pub active_board_id: String,
        pub boards: Vec<BoardSnapshot>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub tool_state: Option<serde_json::Value>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BoardSnapshot {
        pub id: String,
        pub pages: BoardPagesSnapshot,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BoardPagesSnapshot {
        pub pages: Vec<Frame>,
        pub active: usize,
    }

    impl BoardPagesSnapshot {
        fn has_persistable_data(&self) -> bool {
            if self.pages.len() > 1 || self.active > 0 {
                return true;
            }
            self.pages.iter().any(Frame::has_persistable_data)
        }
    }

    impl SessionSnapshot {
        pub fn has_board_data(&self) -> bool {
            self.boards
                .iter()
                .any(|board| board.pages.has_persistable_data())
        }
    }

    pub fn save_snapshot(snapshot: &SessionSnapshot, options: &SessionOptions) -> Result<()> {
        let path = options.session_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create session directory {}", parent.display())
            })?;
        }
        let bytes = serde_json::to_vec_pretty(snapshot).context("failed to encode session json")?;
        fs::write(&path, bytes)
            .with_context(|| format!("failed to write session file {}", path.display()))
    }

    pub fn load_snapshot(options: &SessionOptions) -> Result<Option<SessionSnapshot>> {
        let Some(path) = [
            options.session_file_path(),
            options.backup_file_path(),
            options.recovery_file_path(),
        ]
        .into_iter()
        .find(|path| path.is_file()) else {
            return Ok(None);
        };
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read session file {}", path.display()))?;
        let snapshot = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse session json {}", path.display()))?;
        super::catalog::record_named_session_opened(options);
        Ok(Some(snapshot))
    }
}
