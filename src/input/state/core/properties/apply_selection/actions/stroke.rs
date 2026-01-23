use crate::draw::Shape;
use crate::input::state::PressureThicknessEditMode;
use crate::input::state::core::base::{InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS};
use crate::input::state::core::properties::apply_selection::constants::SELECTION_THICKNESS_STEP;

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_thickness(
        &mut self,
        direction: i32,
    ) -> bool {
        let delta = SELECTION_THICKNESS_STEP * direction as f64;
        let pressure_edit_mode = self.pressure_thickness_edit_mode;
        let pressure_editable = matches!(
            pressure_edit_mode,
            PressureThicknessEditMode::Add | PressureThicknessEditMode::Scale
        );
        let pressure_scale = 1.0 + (self.pressure_thickness_scale_step * direction as f64);
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
                ) || (pressure_editable && matches!(shape, Shape::FreehandPressure { .. }))
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
                Shape::FreehandPressure { points, .. } => match pressure_edit_mode {
                    PressureThicknessEditMode::Disabled => false,
                    PressureThicknessEditMode::Add => {
                        let mut changed = false;
                        for (_, _, thickness) in points.iter_mut() {
                            let next = (*thickness as f64 + delta)
                                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS)
                                as f32;
                            if (next - *thickness).abs() > f32::EPSILON {
                                *thickness = next;
                                changed = true;
                            }
                        }
                        changed
                    }
                    PressureThicknessEditMode::Scale => {
                        if pressure_scale <= 0.0 {
                            return false;
                        }
                        let mut changed = false;
                        for (_, _, thickness) in points.iter_mut() {
                            let next = (*thickness as f64 * pressure_scale)
                                .clamp(MIN_STROKE_THICKNESS, MAX_STROKE_THICKNESS)
                                as f32;
                            if (next - *thickness).abs() > f32::EPSILON {
                                *thickness = next;
                                changed = true;
                            }
                        }
                        changed
                    }
                },
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "thickness")
    }
}
