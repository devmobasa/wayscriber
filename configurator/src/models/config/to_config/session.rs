use super::super::draft::ConfigDraft;
use super::super::parse::{parse_optional_usize_field, parse_u64_field, parse_usize_field};
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_session(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        config.session.persist_transparent = self.session_persist_transparent;
        config.session.persist_whiteboard = self.session_persist_whiteboard;
        config.session.persist_blackboard = self.session_persist_blackboard;
        config.session.persist_history = self.session_persist_history;
        config.session.restore_tool_state = self.session_restore_tool_state;
        config.session.per_output = self.session_per_output;
        config.session.autosave_enabled = self.session_autosave_enabled;
        config.session.storage = self.session_storage_mode.to_mode();
        let custom_dir = self.session_custom_directory.trim();
        config.session.custom_directory = if custom_dir.is_empty() {
            None
        } else {
            Some(custom_dir.to_string())
        };
        parse_usize_field(
            &self.session_max_shapes_per_frame,
            "session.max_shapes_per_frame",
            errors,
            |value| config.session.max_shapes_per_frame = value,
        );
        parse_u64_field(
            &self.session_max_file_size_mb,
            "session.max_file_size_mb",
            errors,
            |value| config.session.max_file_size_mb = value,
        );
        config.session.compress = self.session_compression.to_compression();
        parse_u64_field(
            &self.session_auto_compress_threshold_kb,
            "session.auto_compress_threshold_kb",
            errors,
            |value| config.session.auto_compress_threshold_kb = value,
        );
        parse_optional_usize_field(
            &self.session_max_persisted_undo_depth,
            "session.max_persisted_undo_depth",
            errors,
            |value| config.session.max_persisted_undo_depth = value,
        );
        parse_usize_field(
            &self.session_backup_retention,
            "session.backup_retention",
            errors,
            |value| config.session.backup_retention = value,
        );
        parse_u64_field(
            &self.session_autosave_idle_ms,
            "session.autosave_idle_ms",
            errors,
            |value| config.session.autosave_idle_ms = value,
        );
        parse_u64_field(
            &self.session_autosave_interval_ms,
            "session.autosave_interval_ms",
            errors,
            |value| config.session.autosave_interval_ms = value,
        );
        parse_u64_field(
            &self.session_autosave_failure_backoff_ms,
            "session.autosave_failure_backoff_ms",
            errors,
            |value| config.session.autosave_failure_backoff_ms = value,
        );
    }
}
