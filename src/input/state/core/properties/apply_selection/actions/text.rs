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
