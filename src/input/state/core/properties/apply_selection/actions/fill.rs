use crate::draw::Shape;
use crate::input::state::core::base::{InputState, UiToastKind};

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_fill(
        &mut self,
        direction: i32,
    ) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => Some(*fill),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.set_ui_toast(UiToastKind::Warning, "No fill shapes selected.");
            return false;
        };

        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Rect { .. } | Shape::Ellipse { .. }),
            |shape| match shape {
                Shape::Rect { fill, .. } | Shape::Ellipse { fill, .. } => {
                    if *fill != target {
                        *fill = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "fill")
    }
}
