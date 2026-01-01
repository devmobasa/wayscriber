use crate::draw::Shape;
use crate::input::state::core::base::{InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::state::core::properties::apply_selection::constants::SELECTION_THICKNESS_STEP;

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_thickness(
        &mut self,
        direction: i32,
    ) -> bool {
        let delta = SELECTION_THICKNESS_STEP * direction as f64;
        let result = self.apply_selection_change(
            |shape| {
                matches!(
                    shape,
                    Shape::Freehand { .. }
                        | Shape::Line { .. }
                        | Shape::Rect { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Arrow { .. }
                        | Shape::MarkerStroke { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { thick, .. }
                | Shape::Line { thick, .. }
                | Shape::Rect { thick, .. }
                | Shape::Ellipse { thick, .. }
                | Shape::Arrow { thick, .. }
                | Shape::MarkerStroke { thick, .. } => {
                    let next = (*thick + delta).clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS);
                    if (next - *thick).abs() > f64::EPSILON {
                        *thick = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "thickness")
    }
}
