use crate::draw::Shape;
use crate::input::state::core::base::{InputState, UiToastKind};
use crate::input::state::core::properties::apply_selection::constants::{
    MAX_ARROW_ANGLE, MAX_ARROW_LENGTH, MIN_ARROW_ANGLE, MIN_ARROW_LENGTH,
    SELECTION_ARROW_ANGLE_STEP, SELECTION_ARROW_LENGTH_STEP,
};

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_arrow_head(
        &mut self,
        direction: i32,
    ) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Arrow { head_at_end, .. } => Some(*head_at_end),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No arrows selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { head_at_end, .. } => {
                    if *head_at_end != target {
                        *head_at_end = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow head")
    }

    pub(in crate::input::state::core::properties) fn apply_selection_arrow_length(
        &mut self,
        direction: i32,
    ) -> bool {
        let delta = SELECTION_ARROW_LENGTH_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_length, .. } => {
                    let next = (*arrow_length + delta).clamp(MIN_ARROW_LENGTH, MAX_ARROW_LENGTH);
                    if (next - *arrow_length).abs() > f64::EPSILON {
                        *arrow_length = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow length")
    }

    pub(in crate::input::state::core::properties) fn apply_selection_arrow_angle(
        &mut self,
        direction: i32,
    ) -> bool {
        let delta = SELECTION_ARROW_ANGLE_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Arrow { .. }),
            |shape| match shape {
                Shape::Arrow { arrow_angle, .. } => {
                    let next = (*arrow_angle + delta).clamp(MIN_ARROW_ANGLE, MAX_ARROW_ANGLE);
                    if (next - *arrow_angle).abs() > f64::EPSILON {
                        *arrow_angle = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "arrow angle")
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

    fn add_arrow(
        state: &mut InputState,
        head_at_end: bool,
        arrow_angle: f64,
    ) -> crate::draw::ShapeId {
        state.boards.active_frame_mut().add_shape(Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 10,
            color: state.current_color,
            thick: 3.0,
            arrow_length: 24.0,
            arrow_angle,
            head_at_end,
            label: None,
        })
    }

    #[test]
    fn apply_selection_arrow_head_on_mixed_selection_sets_heads_to_end() {
        let mut state = make_state();
        let first = add_arrow(&mut state, true, 30.0);
        let second = add_arrow(&mut state, false, 30.0);
        state.set_selection(vec![first, second]);

        assert!(state.apply_selection_arrow_head(0));

        for id in [first, second] {
            match &state.boards.active_frame().shape(id).expect("arrow").shape {
                Shape::Arrow { head_at_end, .. } => assert!(*head_at_end),
                other => panic!("expected arrow, got {other:?}"),
            }
        }
    }

    #[test]
    fn apply_selection_arrow_angle_clamps_to_maximum() {
        let mut state = make_state();
        let arrow_id = add_arrow(&mut state, true, MAX_ARROW_ANGLE - 1.0);
        state.set_selection(vec![arrow_id]);

        assert!(state.apply_selection_arrow_angle(1));
        assert!(!state.apply_selection_arrow_angle(1));

        match &state
            .boards
            .active_frame()
            .shape(arrow_id)
            .expect("arrow")
            .shape
        {
            Shape::Arrow { arrow_angle, .. } => assert_eq!(*arrow_angle, MAX_ARROW_ANGLE),
            other => panic!("expected arrow, got {other:?}"),
        }
    }
}
