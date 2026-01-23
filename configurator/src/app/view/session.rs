use iced::widget::{column, pick_list, scrollable, text};
use iced::{Element, Length};

use crate::messages::Message;
use crate::models::{SessionCompressionOption, SessionStorageModeOption, TextField, ToggleField};

use super::super::state::ConfiguratorApp;
use super::widgets::{
    labeled_control, labeled_input, labeled_input_with_feedback, toggle_row, validate_u64_min,
    validate_u64_range, validate_usize_min,
};

impl ConfiguratorApp {
    pub(super) fn session_tab(&self) -> Element<'_, Message> {
        let storage_pick = pick_list(
            SessionStorageModeOption::list(),
            Some(self.draft.session_storage_mode),
            Message::SessionStorageModeChanged,
        );
        let compression_pick = pick_list(
            SessionCompressionOption::list(),
            Some(self.draft.session_compression),
            Message::SessionCompressionChanged,
        );

        let mut column = column![
            text("Session Persistence").size(20),
            toggle_row(
                "Persist transparent mode drawings",
                self.draft.session_persist_transparent,
                self.defaults.session_persist_transparent,
                ToggleField::SessionPersistTransparent,
            ),
            toggle_row(
                "Persist whiteboard mode drawings",
                self.draft.session_persist_whiteboard,
                self.defaults.session_persist_whiteboard,
                ToggleField::SessionPersistWhiteboard,
            ),
            toggle_row(
                "Persist blackboard mode drawings",
                self.draft.session_persist_blackboard,
                self.defaults.session_persist_blackboard,
                ToggleField::SessionPersistBlackboard,
            ),
            toggle_row(
                "Persist undo/redo history",
                self.draft.session_persist_history,
                self.defaults.session_persist_history,
                ToggleField::SessionPersistHistory,
            ),
            toggle_row(
                "Restore tool state on startup",
                self.draft.session_restore_tool_state,
                self.defaults.session_restore_tool_state,
                ToggleField::SessionRestoreToolState,
            ),
            toggle_row(
                "Enable autosave",
                self.draft.session_autosave_enabled,
                self.defaults.session_autosave_enabled,
                ToggleField::SessionAutosaveEnabled,
            ),
            toggle_row(
                "Per-output persistence",
                self.draft.session_per_output,
                self.defaults.session_per_output,
                ToggleField::SessionPerOutput,
            ),
            labeled_control(
                "Storage mode",
                storage_pick.width(Length::Fill).into(),
                self.defaults.session_storage_mode.label().to_string(),
                self.draft.session_storage_mode != self.defaults.session_storage_mode,
            ),
        ]
        .spacing(12);

        if self.draft.session_storage_mode == SessionStorageModeOption::Custom {
            column = column.push(labeled_input(
                "Custom directory",
                &self.draft.session_custom_directory,
                &self.defaults.session_custom_directory,
                TextField::SessionCustomDirectory,
            ));
        }

        column = column
            .push(labeled_control(
                "Compression",
                compression_pick.width(Length::Fill).into(),
                self.defaults.session_compression.label().to_string(),
                self.draft.session_compression != self.defaults.session_compression,
            ))
            .push(labeled_input_with_feedback(
                "Autosave idle (ms)",
                &self.draft.session_autosave_idle_ms,
                &self.defaults.session_autosave_idle_ms,
                TextField::SessionAutosaveIdleMs,
                Some("Minimum: 1000 ms"),
                validate_u64_min(&self.draft.session_autosave_idle_ms, 1000),
            ))
            .push(labeled_input_with_feedback(
                "Autosave interval (ms)",
                &self.draft.session_autosave_interval_ms,
                &self.defaults.session_autosave_interval_ms,
                TextField::SessionAutosaveIntervalMs,
                Some("Minimum: 1000 ms"),
                validate_u64_min(&self.draft.session_autosave_interval_ms, 1000),
            ))
            .push(labeled_input_with_feedback(
                "Autosave failure backoff (ms)",
                &self.draft.session_autosave_failure_backoff_ms,
                &self.defaults.session_autosave_failure_backoff_ms,
                TextField::SessionAutosaveFailureBackoffMs,
                Some("Minimum: 1000 ms"),
                validate_u64_min(&self.draft.session_autosave_failure_backoff_ms, 1000),
            ))
            .push(labeled_input_with_feedback(
                "Max shapes per frame",
                &self.draft.session_max_shapes_per_frame,
                &self.defaults.session_max_shapes_per_frame,
                TextField::SessionMaxShapesPerFrame,
                Some("Minimum: 1"),
                validate_usize_min(&self.draft.session_max_shapes_per_frame, 1),
            ))
            .push(labeled_input(
                "Max persisted undo depth (blank = runtime limit)",
                &self.draft.session_max_persisted_undo_depth,
                &self.defaults.session_max_persisted_undo_depth,
                TextField::SessionMaxPersistedUndoDepth,
            ))
            .push(labeled_input_with_feedback(
                "Max file size (MB)",
                &self.draft.session_max_file_size_mb,
                &self.defaults.session_max_file_size_mb,
                TextField::SessionMaxFileSizeMb,
                Some("Range: 1-1024 MB"),
                validate_u64_range(&self.draft.session_max_file_size_mb, 1, 1024),
            ))
            .push(labeled_input_with_feedback(
                "Auto-compress threshold (KB)",
                &self.draft.session_auto_compress_threshold_kb,
                &self.defaults.session_auto_compress_threshold_kb,
                TextField::SessionAutoCompressThresholdKb,
                Some("Minimum: 1 KB"),
                validate_u64_min(&self.draft.session_auto_compress_threshold_kb, 1),
            ))
            .push(labeled_input(
                "Backup retention count",
                &self.draft.session_backup_retention,
                &self.defaults.session_backup_retention,
                TextField::SessionBackupRetention,
            ));

        scrollable(column).into()
    }
}
