use super::Config;

impl Config {
    pub(super) fn validate_history(&mut self) {
        // History delays: clamp to a reasonable range to avoid long freezes or instant drains.
        const MAX_DELAY_MS: u64 = 5_000;
        const MIN_DELAY_MS: u64 = 50;
        let clamp_delay = |label: &str, value: &mut u64| {
            if *value < MIN_DELAY_MS {
                log::warn!(
                    "{} {}ms too small; clamping to {}ms",
                    label,
                    *value,
                    MIN_DELAY_MS
                );
                *value = MIN_DELAY_MS;
            }
            if *value > MAX_DELAY_MS {
                log::warn!(
                    "{} {}ms too large; clamping to {}ms",
                    label,
                    *value,
                    MAX_DELAY_MS
                );
                *value = MAX_DELAY_MS;
            }
        };
        clamp_delay("undo_all_delay_ms", &mut self.history.undo_all_delay_ms);
        clamp_delay("redo_all_delay_ms", &mut self.history.redo_all_delay_ms);
        clamp_delay(
            "custom_undo_delay_ms",
            &mut self.history.custom_undo_delay_ms,
        );
        clamp_delay(
            "custom_redo_delay_ms",
            &mut self.history.custom_redo_delay_ms,
        );

        // Custom history step counts: clamp to sane bounds.
        const MIN_STEPS: usize = 1;
        const MAX_STEPS: usize = 500;
        let clamp_steps = |label: &str, value: &mut usize| {
            if *value < MIN_STEPS {
                log::warn!("{} {} too small; clamping to {}", label, *value, MIN_STEPS);
                *value = MIN_STEPS;
            }
            if *value > MAX_STEPS {
                log::warn!("{} {} too large; clamping to {}", label, *value, MAX_STEPS);
                *value = MAX_STEPS;
            }
        };
        clamp_steps("custom_undo_steps", &mut self.history.custom_undo_steps);
        clamp_steps("custom_redo_steps", &mut self.history.custom_redo_steps);
    }
}
