use crate::input::InputState;
use crate::ui::toolbar::model::ToolbarSliderSpec;

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
    let spec = ToolbarSliderSpec::DELAY_SECONDS;
    (delay_secs.clamp(spec.min, spec.max) * 1000.0).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::from_seed(crate::input::InputStateSeed {
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thickness: 4.0,
            eraser_size: 4.0,
            eraser_mode: EraserMode::Brush,
            marker_opacity: 0.32,
            fill_enabled: false,
            font_size: 32.0,
            font_descriptor: FontDescriptor::default(),
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: false,
            show_status_bar: true,
            boards_config: BoardsConfig::default(),
            action_map: action_map,
            max_shapes_per_frame: usize::MAX,
            click_highlight_settings: ClickHighlightSettings::disabled(),
            undo_all_delay_ms: 0,
            redo_all_delay_ms: 0,
            custom_section_enabled: true,
            custom_undo_delay_ms: 0,
            custom_redo_delay_ms: 0,
            custom_undo_steps: 5,
            custom_redo_steps: 5,
            presenter_mode_config: PresenterModeConfig::default(),
        })
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

        assert_eq!(state.undo_all_delay_ms, 50);
        assert_eq!(state.redo_all_delay_ms, 2345);
        assert_eq!(state.custom_undo_delay_ms, 5000);
        assert_eq!(state.custom_redo_delay_ms, 333);
    }

    #[test]
    fn custom_undo_steps_clamp_and_report_when_value_changes() {
        let mut state = make_state();
        state.custom_undo_steps = 5;

        assert!(state.apply_toolbar_set_custom_undo_steps(0));
        assert_eq!(state.custom_undo_steps, 1);
        assert!(state.apply_toolbar_set_custom_undo_steps(999));
        assert_eq!(state.custom_undo_steps, 500);
        assert!(!state.apply_toolbar_set_custom_undo_steps(500));
    }

    #[test]
    fn custom_redo_steps_clamp_and_report_when_value_changes() {
        let mut state = make_state();
        state.custom_redo_steps = 5;

        assert!(state.apply_toolbar_set_custom_redo_steps(0));
        assert_eq!(state.custom_redo_steps, 1);
        assert!(state.apply_toolbar_set_custom_redo_steps(999));
        assert_eq!(state.custom_redo_steps, 500);
        assert!(!state.apply_toolbar_set_custom_redo_steps(500));
    }
}
