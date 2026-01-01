use crate::input::InputState;

const MIN_DELAY_S: f64 = 0.05;
const MAX_DELAY_S: f64 = 5.0;

impl InputState {
    pub(super) fn apply_toolbar_set_undo_delay(&mut self, delay_secs: f64) -> bool {
        self.undo_all_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_redo_delay(&mut self, delay_secs: f64) -> bool {
        self.redo_all_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_custom_undo_delay(&mut self, delay_secs: f64) -> bool {
        self.custom_undo_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_custom_redo_delay(&mut self, delay_secs: f64) -> bool {
        self.custom_redo_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_custom_undo_steps(&mut self, steps: usize) -> bool {
        let clamped = steps.clamp(1, 500);
        if self.custom_undo_steps != clamped {
            self.custom_undo_steps = clamped;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_set_custom_redo_steps(&mut self, steps: usize) -> bool {
        let clamped = steps.clamp(1, 500);
        if self.custom_redo_steps != clamped {
            self.custom_redo_steps = clamped;
            true
        } else {
            false
        }
    }
}

fn clamp_delay_ms(delay_secs: f64) -> u64 {
    (delay_secs.clamp(MIN_DELAY_S, MAX_DELAY_S) * 1000.0).round() as u64
}
