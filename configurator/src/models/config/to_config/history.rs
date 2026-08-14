use super::super::draft::ConfigDraft;
use super::super::parse::{parse_u64_in_range, parse_usize_in_range};
use crate::models::error::FormError;
use wayscriber::config::Config;

const HISTORY_DELAY_MS_MIN: u64 = 50;
const HISTORY_DELAY_MS_MAX: u64 = 5_000;
const HISTORY_STEPS_MIN: usize = 1;
const HISTORY_STEPS_MAX: usize = 500;

impl ConfigDraft {
    pub(super) fn apply_history(&self, config: &mut Config, errors: &mut Vec<FormError>) {
        parse_u64_in_range(
            &self.history_undo_all_delay_ms,
            "history.undo_all_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.history.undo_all_delay_ms = value,
        );
        parse_u64_in_range(
            &self.history_redo_all_delay_ms,
            "history.redo_all_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.history.redo_all_delay_ms = value,
        );
        config.history.custom_section_enabled = self.history_custom_section_enabled;
        parse_u64_in_range(
            &self.history_custom_undo_delay_ms,
            "history.custom_undo_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.history.custom_undo_delay_ms = value,
        );
        parse_u64_in_range(
            &self.history_custom_redo_delay_ms,
            "history.custom_redo_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.history.custom_redo_delay_ms = value,
        );
        parse_usize_in_range(
            &self.history_custom_undo_steps,
            "history.custom_undo_steps",
            HISTORY_STEPS_MIN,
            HISTORY_STEPS_MAX,
            errors,
            |value| config.history.custom_undo_steps = value,
        );
        parse_usize_in_range(
            &self.history_custom_redo_steps,
            "history.custom_redo_steps",
            HISTORY_STEPS_MIN,
            HISTORY_STEPS_MAX,
            errors,
            |value| config.history.custom_redo_steps = value,
        );
    }
}
