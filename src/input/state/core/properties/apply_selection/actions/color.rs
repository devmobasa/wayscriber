use crate::draw::{Color, RED, Shape};
use crate::input::state::core::base::InputState;
use crate::input::state::core::properties::utils::{
    SELECTION_COLORS, color_palette_index, cycle_index,
};

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
}
