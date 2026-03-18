use crate::draw::{Color, RED, Shape};
use crate::input::state::core::base::InputState;
use crate::input::state::core::properties::utils::{
    SELECTION_COLORS, color_palette_index, cycle_index,
};

impl InputState {
    pub(crate) fn apply_selection_color_value(&mut self, target: Color) -> bool {
        let result = self.apply_selection_change(
            |shape| {
                matches!(
                    shape,
                    Shape::Freehand { .. }
                        | Shape::FreehandPressure { .. }
                        | Shape::Line { .. }
                        | Shape::Rect { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Arrow { .. }
                        | Shape::MarkerStroke { .. }
                        | Shape::Text { .. }
                        | Shape::StepMarker { .. }
                        | Shape::StickyNote { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { color, .. }
                | Shape::FreehandPressure { color, .. }
                | Shape::Line { color, .. }
                | Shape::Rect { color, .. }
                | Shape::Ellipse { color, .. }
                | Shape::Arrow { color, .. }
                | Shape::Text { color, .. }
                | Shape::StepMarker { color, .. } => {
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
                        | Shape::FreehandPressure { .. }
                        | Shape::Line { .. }
                        | Shape::Rect { .. }
                        | Shape::Ellipse { .. }
                        | Shape::Arrow { .. }
                        | Shape::MarkerStroke { .. }
                        | Shape::Text { .. }
                        | Shape::StepMarker { .. }
                        | Shape::StickyNote { .. }
                )
            },
            |shape| match shape {
                Shape::Freehand { color, .. }
                | Shape::FreehandPressure { color, .. }
                | Shape::Line { color, .. }
                | Shape::Rect { color, .. }
                | Shape::Ellipse { color, .. }
                | Shape::Arrow { color, .. }
                | Shape::Text { color, .. }
                | Shape::StepMarker { color, .. } => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::FontDescriptor;
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
    fn apply_selection_color_value_preserves_marker_alpha() {
        let mut state = make_state();
        let marker_id = state.boards.active_frame_mut().add_shape(Shape::MarkerStroke {
            points: vec![(0, 0), (10, 10)],
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 0.25,
            },
            thick: 8.0,
        });
        state.set_selection(vec![marker_id]);

        assert!(state.apply_selection_color_value(RED));

        match &state.boards.active_frame().shape(marker_id).expect("marker").shape {
            Shape::MarkerStroke { color, .. } => assert_eq!(
                *color,
                Color {
                    r: RED.r,
                    g: RED.g,
                    b: RED.b,
                    a: 0.25,
                }
            ),
            other => panic!("expected marker stroke, got {other:?}"),
        }
    }

    #[test]
    fn apply_selection_color_wraps_palette_forward_from_black_to_red() {
        let mut state = make_state();
        let rect_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            color: crate::draw::BLACK,
            thick: 2.0,
        });
        state.set_selection(vec![rect_id]);

        assert!(state.apply_selection_color(0));

        match &state.boards.active_frame().shape(rect_id).expect("rect").shape {
            Shape::Rect { color, .. } => assert_eq!(*color, RED),
            other => panic!("expected rect, got {other:?}"),
        }
    }
}
