use crate::draw::{SPOTLIGHT_MAGNIFICATION_STEP, Shape};
use crate::input::state::core::base::InputState;

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_spotlight_magnification(
        &mut self,
        direction: i32,
    ) -> bool {
        let delta = SPOTLIGHT_MAGNIFICATION_STEP * f64::from(direction);
        let result = self.apply_selection_change(
            |shape| matches!(shape, Shape::Spotlight { .. }),
            |shape| match shape {
                Shape::Spotlight { magnification, .. } => {
                    let next =
                        crate::draw::normalize_spotlight_magnification(*magnification + delta);
                    if (next - *magnification).abs() > f64::EPSILON {
                        *magnification = next;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "Spotlight magnification")
    }
}
