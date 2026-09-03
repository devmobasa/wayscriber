use crate::input::InputState;
use crate::ui::toolbar::model::ToolbarSliderSpec;

impl InputState {
    pub(super) fn apply_toolbar_set_undo_delay(&mut self, delay_secs: f64) -> bool {
        self.history_limits.undo_all_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_redo_delay(&mut self, delay_secs: f64) -> bool {
        self.history_limits.redo_all_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_custom_undo_delay(&mut self, delay_secs: f64) -> bool {
        self.history_limits.custom_undo_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_custom_redo_delay(&mut self, delay_secs: f64) -> bool {
        self.history_limits.custom_redo_delay_ms = clamp_delay_ms(delay_secs);
        true
    }

    pub(super) fn apply_toolbar_set_custom_undo_steps(&mut self, steps: usize) -> bool {
        let clamped = steps.clamp(1, 500);
        if self.history_limits.custom_undo_steps != clamped {
            self.history_limits.custom_undo_steps = clamped;
            true
        } else {
            false
        }
    }

    pub(super) fn apply_toolbar_set_custom_redo_steps(&mut self, steps: usize) -> bool {
        let clamped = steps.clamp(1, 500);
        if self.history_limits.custom_redo_steps != clamped {
            self.history_limits.custom_redo_steps = clamped;
            true
        } else {
            false
        }
    }
}

fn clamp_delay_ms(delay_secs: f64) -> u64 {
    let spec = ToolbarSliderSpec::DELAY_SECONDS;
    (delay_secs.clamp(spec.min, spec.max) * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let _action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        crate::input::state::test_support::make_test_input_state()
    }

    #[test]
    fn clamp_delay_ms_clamps_to_minimum_and_maximum_bounds() {
        assert_eq!(clamp_delay_ms(0.0), 50);
        assert_eq!(clamp_delay_ms(99.0), 5000);
    }

    #[test]
    fn apply_toolbar_set_delay_methods_store_clamped_milliseconds() {
        let mut state = make_state();

        assert!(state.apply_toolbar_set_undo_delay(0.01));
        assert!(state.apply_toolbar_set_redo_delay(2.345));
        assert!(state.apply_toolbar_set_custom_undo_delay(8.0));
        assert!(state.apply_toolbar_set_custom_redo_delay(0.333));

        assert_eq!(state.history_limits.undo_all_delay_ms, 50);
        assert_eq!(state.history_limits.redo_all_delay_ms, 2345);
        assert_eq!(state.history_limits.custom_undo_delay_ms, 5000);
        assert_eq!(state.history_limits.custom_redo_delay_ms, 333);
    }

    #[test]
    fn custom_undo_steps_clamp_and_report_when_value_changes() {
        let mut state = make_state();
        state.history_limits.custom_undo_steps = 5;

        assert!(state.apply_toolbar_set_custom_undo_steps(0));
        assert_eq!(state.history_limits.custom_undo_steps, 1);
        assert!(state.apply_toolbar_set_custom_undo_steps(999));
        assert_eq!(state.history_limits.custom_undo_steps, 500);
        assert!(!state.apply_toolbar_set_custom_undo_steps(500));
    }

    #[test]
    fn custom_redo_steps_clamp_and_report_when_value_changes() {
        let mut state = make_state();
        state.history_limits.custom_redo_steps = 5;

        assert!(state.apply_toolbar_set_custom_redo_steps(0));
        assert_eq!(state.history_limits.custom_redo_steps, 1);
        assert!(state.apply_toolbar_set_custom_redo_steps(999));
        assert_eq!(state.history_limits.custom_redo_steps, 500);
        assert!(!state.apply_toolbar_set_custom_redo_steps(500));
    }
}
