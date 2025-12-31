use std::path::PathBuf;
use std::time::SystemTime;

/// Result of clearing on-disk session data.
#[derive(Debug, Clone, Copy)]
pub struct ClearOutcome {
    pub removed_session: bool,
    pub removed_backup: bool,
    pub removed_lock: bool,
}

/// Summary information about the current session file(s).
#[derive(Debug, Clone)]
pub struct SessionInspection {
    pub session_path: PathBuf,
    pub exists: bool,
    pub size_bytes: Option<u64>,
    pub modified: Option<SystemTime>,
    pub backup_path: PathBuf,
    pub backup_exists: bool,
    pub backup_size_bytes: Option<u64>,
    pub active_identity: Option<String>,
    pub per_output: bool,
    pub persist_transparent: bool,
    pub persist_whiteboard: bool,
    pub persist_blackboard: bool,
    pub persist_history: bool,
    pub restore_tool_state: bool,
    pub history_limit: Option<usize>,
    pub frame_counts: Option<FrameCounts>,
    pub history_counts: Option<HistoryCounts>,
    pub history_present: bool,
    pub tool_state_present: bool,
    pub compressed: bool,
    pub file_version: Option<u32>,
}

/// Frame counts for each board stored in the session.
#[derive(Debug, Clone, Copy)]
pub struct FrameCounts {
    pub transparent: usize,
    pub whiteboard: usize,
    pub blackboard: usize,
}

/// Undo/redo counts for each board stored in the session.
#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryCounts {
    pub transparent: HistoryDepth,
    pub whiteboard: HistoryDepth,
    pub blackboard: HistoryDepth,
}

impl HistoryCounts {
    pub(super) fn has_history(&self) -> bool {
        self.transparent.has_history()
            || self.whiteboard.has_history()
            || self.blackboard.has_history()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HistoryDepth {
    pub undo: usize,
    pub redo: usize,
}

impl HistoryDepth {
    pub(super) fn has_history(&self) -> bool {
        self.undo > 0 || self.redo > 0
    }
}
