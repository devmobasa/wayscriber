use super::base::DelayedHistory;
use crate::config::HistoryConfig;

/// Undo retention, delayed playback settings, and active playback state.
#[derive(Clone)]
pub(crate) struct HistoryLimits {
    pub(crate) undo_stack_limit: usize,
    pub(crate) undo_all_delay_ms: u64,
    pub(crate) redo_all_delay_ms: u64,
    pub(crate) custom_undo_delay_ms: u64,
    pub(crate) custom_redo_delay_ms: u64,
    pub(crate) custom_undo_steps: usize,
    pub(crate) custom_redo_steps: usize,
    pub(crate) custom_section_enabled: bool,
    pub(crate) pending_history: Option<DelayedHistory>,
}

impl From<&HistoryConfig> for HistoryLimits {
    fn from(config: &HistoryConfig) -> Self {
        Self {
            undo_stack_limit: 100,
            undo_all_delay_ms: config.undo_all_delay_ms,
            redo_all_delay_ms: config.redo_all_delay_ms,
            custom_undo_delay_ms: config.custom_undo_delay_ms,
            custom_redo_delay_ms: config.custom_redo_delay_ms,
            custom_undo_steps: config.custom_undo_steps,
            custom_redo_steps: config.custom_redo_steps,
            custom_section_enabled: config.custom_section_enabled,
            pending_history: None,
        }
    }
}

impl Default for HistoryLimits {
    fn default() -> Self {
        Self::from(&HistoryConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_config_maps_every_playback_setting() {
        let config = HistoryConfig {
            undo_all_delay_ms: 11,
            redo_all_delay_ms: 22,
            custom_section_enabled: true,
            custom_undo_delay_ms: 33,
            custom_redo_delay_ms: 44,
            custom_undo_steps: 5,
            custom_redo_steps: 6,
        };

        let limits = HistoryLimits::from(&config);

        assert_eq!(limits.undo_stack_limit, 100);
        assert_eq!(limits.undo_all_delay_ms, 11);
        assert_eq!(limits.redo_all_delay_ms, 22);
        assert_eq!(limits.custom_undo_delay_ms, 33);
        assert_eq!(limits.custom_redo_delay_ms, 44);
        assert_eq!(limits.custom_undo_steps, 5);
        assert_eq!(limits.custom_redo_steps, 6);
        assert!(limits.custom_section_enabled);
        assert!(limits.pending_history.is_none());
    }
}
