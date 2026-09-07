use super::parse::{parse_u64_in_range, parse_usize_in_range};
use crate::models::error::FormError;
use wayscriber::config::HistoryConfig;

const HISTORY_DELAY_MS_MIN: u64 = 50;
const HISTORY_DELAY_MS_MAX: u64 = 5_000;
const HISTORY_STEPS_MIN: usize = 1;
const HISTORY_STEPS_MAX: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryDraft {
    pub undo_all_delay_ms: String,
    pub redo_all_delay_ms: String,
    pub custom_section_enabled: bool,
    pub custom_undo_delay_ms: String,
    pub custom_redo_delay_ms: String,
    pub custom_undo_steps: String,
    pub custom_redo_steps: String,
}

impl HistoryDraft {
    pub(super) fn from_config(config: &HistoryConfig) -> Self {
        Self {
            undo_all_delay_ms: config.undo_all_delay_ms.to_string(),
            redo_all_delay_ms: config.redo_all_delay_ms.to_string(),
            custom_section_enabled: config.custom_section_enabled,
            custom_undo_delay_ms: config.custom_undo_delay_ms.to_string(),
            custom_redo_delay_ms: config.custom_redo_delay_ms.to_string(),
            custom_undo_steps: config.custom_undo_steps.to_string(),
            custom_redo_steps: config.custom_redo_steps.to_string(),
        }
    }

    pub(super) fn apply_to(&self, config: &mut HistoryConfig, errors: &mut Vec<FormError>) {
        parse_u64_in_range(
            &self.undo_all_delay_ms,
            "history.undo_all_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.undo_all_delay_ms = value,
        );
        parse_u64_in_range(
            &self.redo_all_delay_ms,
            "history.redo_all_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.redo_all_delay_ms = value,
        );
        config.custom_section_enabled = self.custom_section_enabled;
        parse_u64_in_range(
            &self.custom_undo_delay_ms,
            "history.custom_undo_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.custom_undo_delay_ms = value,
        );
        parse_u64_in_range(
            &self.custom_redo_delay_ms,
            "history.custom_redo_delay_ms",
            HISTORY_DELAY_MS_MIN,
            HISTORY_DELAY_MS_MAX,
            errors,
            |value| config.custom_redo_delay_ms = value,
        );
        parse_usize_in_range(
            &self.custom_undo_steps,
            "history.custom_undo_steps",
            HISTORY_STEPS_MIN,
            HISTORY_STEPS_MAX,
            errors,
            |value| config.custom_undo_steps = value,
        );
        parse_usize_in_range(
            &self.custom_redo_steps,
            "history.custom_redo_steps",
            HISTORY_STEPS_MIN,
            HISTORY_STEPS_MAX,
            errors,
            |value| config.custom_redo_steps = value,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_section_round_trips_and_retains_invalid_text() {
        let original = HistoryConfig::default();
        let mut draft = HistoryDraft::from_config(&original);
        let mut saved = original.clone();
        let mut errors = Vec::new();
        draft.apply_to(&mut saved, &mut errors);
        assert!(errors.is_empty());
        assert_eq!(HistoryDraft::from_config(&saved), draft);

        draft.custom_undo_delay_ms = "-".into();
        draft.custom_redo_steps = "0".into();
        draft.apply_to(&mut saved, &mut errors);
        assert_eq!(errors.len(), 2);
        assert_eq!(draft.custom_undo_delay_ms, "-");
        assert_eq!(saved.custom_undo_delay_ms, original.custom_undo_delay_ms);
        assert_eq!(saved.custom_redo_steps, original.custom_redo_steps);
    }
}
