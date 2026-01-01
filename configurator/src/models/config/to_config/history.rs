use super::super::draft::ConfigDraft;
use super::super::parse::{parse_u64_field, parse_usize_field};
use crate::models::error::FormError;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_history(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        parse_u64_field(
            &self.history_undo_all_delay_ms,
            "history.undo_all_delay_ms",
            errors,
            |value| config.history.undo_all_delay_ms = value,
        );
        parse_u64_field(
            &self.history_redo_all_delay_ms,
            "history.redo_all_delay_ms",
            errors,
            |value| config.history.redo_all_delay_ms = value,
        );
        config.history.custom_section_enabled = self.history_custom_section_enabled;
        parse_u64_field(
            &self.history_custom_undo_delay_ms,
            "history.custom_undo_delay_ms",
            errors,
            |value| config.history.custom_undo_delay_ms = value,
        );
        parse_u64_field(
            &self.history_custom_redo_delay_ms,
            "history.custom_redo_delay_ms",
            errors,
            |value| config.history.custom_redo_delay_ms = value,
        );
        parse_usize_field(
            &self.history_custom_undo_steps,
            "history.custom_undo_steps",
            errors,
            |value| config.history.custom_undo_steps = value,
        );
        parse_usize_field(
            &self.history_custom_redo_steps,
            "history.custom_redo_steps",
            errors,
            |value| config.history.custom_redo_steps = value,
        );
    }
}
