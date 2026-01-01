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
