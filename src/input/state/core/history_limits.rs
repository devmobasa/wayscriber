use crate::config::HistoryConfig;
use crate::ui::toolbar::model::ToolbarSliderSpec;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct DelayedHistory {
    mode: HistoryMode,
    remaining: usize,
    delay_ms: u64,
    next_due: Instant,
}

#[derive(Clone, Copy)]
pub(super) enum HistoryMode {
    Undo,
    Redo,
}

/// Undo retention, delayed playback settings, and active playback state.
#[derive(Clone)]
pub(crate) struct HistoryLimits {
    undo_stack_limit: usize,
    undo_all_delay_ms: u64,
    redo_all_delay_ms: u64,
    custom_undo_delay_ms: u64,
    custom_redo_delay_ms: u64,
    custom_undo_steps: usize,
    custom_redo_steps: usize,
    custom_section_enabled: bool,
    pending_history: Option<DelayedHistory>,
}

impl HistoryLimits {
    const MIN_DELAY_MS: u64 = 50;

    pub(crate) fn undo_stack_limit(&self) -> usize {
        self.undo_stack_limit
    }

    pub(crate) fn undo_all_delay_ms(&self) -> u64 {
        self.undo_all_delay_ms
    }

    pub(crate) fn redo_all_delay_ms(&self) -> u64 {
        self.redo_all_delay_ms
    }

    pub(crate) fn custom_undo_delay_ms(&self) -> u64 {
        self.custom_undo_delay_ms
    }

    pub(crate) fn custom_redo_delay_ms(&self) -> u64 {
        self.custom_redo_delay_ms
    }

    pub(crate) fn custom_undo_steps(&self) -> usize {
        self.custom_undo_steps
    }

    pub(crate) fn custom_redo_steps(&self) -> usize {
        self.custom_redo_steps
    }

    pub(crate) fn custom_section_enabled(&self) -> bool {
        self.custom_section_enabled
    }

    pub(crate) fn set_custom_section_enabled(&mut self, enabled: bool) {
        self.custom_section_enabled = enabled;
    }

    pub(crate) fn set_undo_stack_limit(&mut self, limit: usize) {
        self.undo_stack_limit = limit.max(1);
    }

    pub(crate) fn set_undo_all_delay(&mut self, delay_secs: f64) {
        self.undo_all_delay_ms = Self::clamp_delay_ms(delay_secs);
    }

    pub(crate) fn set_redo_all_delay(&mut self, delay_secs: f64) {
        self.redo_all_delay_ms = Self::clamp_delay_ms(delay_secs);
    }

    pub(crate) fn set_custom_undo_delay(&mut self, delay_secs: f64) {
        self.custom_undo_delay_ms = Self::clamp_delay_ms(delay_secs);
    }

    pub(crate) fn set_custom_redo_delay(&mut self, delay_secs: f64) {
        self.custom_redo_delay_ms = Self::clamp_delay_ms(delay_secs);
    }

    pub(crate) fn set_custom_undo_steps(&mut self, steps: usize) -> bool {
        let clamped = steps.clamp(1, 500);
        if self.custom_undo_steps == clamped {
            return false;
        }
        self.custom_undo_steps = clamped;
        true
    }

    pub(crate) fn set_custom_redo_steps(&mut self, steps: usize) -> bool {
        let clamped = steps.clamp(1, 500);
        if self.custom_redo_steps == clamped {
            return false;
        }
        self.custom_redo_steps = clamped;
        true
    }

    fn clamp_delay_ms(delay_secs: f64) -> u64 {
        let spec = ToolbarSliderSpec::DELAY_SECONDS;
        (delay_secs.clamp(spec.min, spec.max) * 1000.0).round() as u64
    }

    pub(super) fn has_pending(&self) -> bool {
        self.pending_history.is_some()
    }

    pub(super) fn schedule(
        &mut self,
        mode: HistoryMode,
        available: usize,
        requested_steps: usize,
        delay_ms: u64,
        now: Instant,
    ) -> bool {
        let remaining = available.min(requested_steps);
        if remaining == 0 {
            return false;
        }
        self.pending_history = Some(DelayedHistory {
            mode,
            remaining,
            delay_ms: delay_ms.max(Self::MIN_DELAY_MS),
            next_due: now,
        });
        true
    }

    pub(super) fn due_mode(&self, now: Instant) -> Option<HistoryMode> {
        self.pending_history
            .as_ref()
            .filter(|pending| pending.remaining > 0 && now >= pending.next_due)
            .map(|pending| pending.mode)
    }

    pub(super) fn finish_due_step(&mut self, now: Instant, succeeded: bool) {
        let Some(pending) = self.pending_history.as_mut() else {
            return;
        };
        if succeeded {
            pending.remaining = pending.remaining.saturating_sub(1);
            pending.next_due = now + Duration::from_millis(pending.delay_ms);
        } else {
            pending.remaining = 0;
        }
        if pending.remaining == 0 {
            self.pending_history = None;
        }
    }
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
    fn toolbar_values_are_clamped_by_the_owner() {
        let mut limits = HistoryLimits::default();

        limits.set_undo_all_delay(0.0);
        limits.set_redo_all_delay(99.0);
        assert_eq!(limits.undo_all_delay_ms, 50);
        assert_eq!(limits.redo_all_delay_ms, 5000);

        assert!(limits.set_custom_undo_steps(0));
        assert_eq!(limits.custom_undo_steps, 1);
        assert!(limits.set_custom_redo_steps(999));
        assert_eq!(limits.custom_redo_steps, 500);
        assert!(!limits.set_custom_redo_steps(500));
    }

    #[test]
    fn delayed_history_schedule_owns_due_and_completion_transitions() {
        let mut limits = HistoryLimits::default();
        let now = Instant::now();

        assert!(limits.schedule(HistoryMode::Undo, 3, 8, 0, now));
        assert!(matches!(limits.due_mode(now), Some(HistoryMode::Undo)));
        assert_eq!(
            limits
                .pending_history
                .as_ref()
                .map(|pending| pending.remaining),
            Some(3)
        );

        limits.finish_due_step(now, true);
        assert!(limits.has_pending());
        assert!(limits.due_mode(now).is_none());
        limits.finish_due_step(now, false);
        assert!(!limits.has_pending());
    }

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
