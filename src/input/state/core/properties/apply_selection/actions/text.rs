use crate::draw::Shape;
use crate::input::state::core::base::{InputState, UiToastKind};
use crate::input::state::core::properties::apply_selection::constants::{
    MAX_FONT_SIZE, MIN_FONT_SIZE, SELECTION_FONT_SIZE_STEP,
};

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_font_size(
        &mut self,
        direction: i32,
    ) -> bool {
        let delta = SELECTION_FONT_SIZE_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Text { .. }),
            |shape| match shape {
                Shape::Text { size, .. } => {
                    let next = (*size + delta).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
                    if (next - *size).abs() > f64::EPSILON {
                        *size = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "font size")
    }

    pub(in crate::input::state::core::properties) fn apply_selection_text_background(
        &mut self,
        direction: i32,
    ) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Text {
                    background_enabled, ..
                } => Some(*background_enabled),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No text shapes selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Text { .. }),
            |shape| match shape {
                Shape::Text {
                    background_enabled, ..
                } => {
                    if *background_enabled != target {
                        *background_enabled = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "text background")
    }
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

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            4.0,
            4.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor::default(),
            false,
            20.0,
            30.0,
            false,
            true,
            BoardsConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            true,
            0,
            0,
            5,
            5,
            PresenterModeConfig::default(),
        )
    }

    #[test]
    fn apply_selection_text_background_warns_when_no_text_shapes_are_selected() {
        let mut state = make_state();
        let rect_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            color: state.current_color,
            thick: 2.0,
        });
        state.set_selection(vec![rect_id]);

        assert!(!state.apply_selection_text_background(0));
        assert_eq!(
            state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
            Some("No text shapes selected.")
        );
    }

    #[test]
    fn apply_selection_font_size_clamps_to_maximum() {
        let mut state = make_state();
        let text_id = state.boards.active_frame_mut().add_shape(Shape::Text {
            x: 10,
            y: 20,
            text: "Note".to_string(),
            color: state.current_color,
            size: MAX_FONT_SIZE - 1.0,
            font_descriptor: state.font_descriptor.clone(),
            background_enabled: false,
            wrap_width: None,
        });
        state.set_selection(vec![text_id]);

        assert!(state.apply_selection_font_size(1));
        assert!(!state.apply_selection_font_size(1));

        match &state
            .boards
            .active_frame()
            .shape(text_id)
            .expect("text")
            .shape
        {
            Shape::Text { size, .. } => assert_eq!(*size, MAX_FONT_SIZE),
            other => panic!("expected text, got {other:?}"),
        }
    }
}
