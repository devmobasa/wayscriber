use super::super::super::base::{
    InputState, MAX_STROKE_THICKNESS, MIN_STROKE_THICKNESS, UiToastKind,
};
use super::super::utils::{SELECTION_COLORS, color_palette_index, cycle_index};
use super::constants::{
    MAX_ARROW_ANGLE, MAX_ARROW_LENGTH, MAX_FONT_SIZE, MIN_ARROW_ANGLE, MIN_ARROW_LENGTH,
    MIN_FONT_SIZE, SELECTION_ARROW_ANGLE_STEP, SELECTION_ARROW_LENGTH_STEP,
    SELECTION_FONT_SIZE_STEP, SELECTION_THICKNESS_STEP,
};
use crate::draw::{Color, RED, Shape};

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_color(
        &mut self,
        direction: i32,
    ) -> bool {
        let base_color = self.selection_primary_color().unwrap_or(RED);
        let index = color_palette_index(base_color).unwrap_or(0);
        let offset = if direction == 0 { 1 } else { direction };
        let next = cycle_index(index, SELECTION_COLORS.len(), offset);
        let target = SELECTION_COLORS[next].1;

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
                        | Shape::Text { .. }
                        | Shape::StickyNote { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { color, .. }
                | Shape::Line { color, .. }
                | Shape::Rect { color, .. }
                | Shape::Ellipse { color, .. }
                | Shape::Arrow { color, .. }
                | Shape::Text { color, .. } => {
                    if *color != target {
                        *color = target;
                        true
                    } else {
                        false
                    }
                }
                Shape::MarkerStroke { color, .. } => {
                    let new_color = Color {
                        a: color.a,
                        ..target
                    };
                    if *color != new_color {
                        *color = new_color;
                        true
                    } else {
                        false
                    }
                }
                Shape::StickyNote { background, .. } => {
                    if *background != target {
                        *background = target;
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "color")
    }

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
