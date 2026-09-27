use crate::draw::TextMeasurer;
use crate::draw::{Color, Shape};
use crate::input::state::core::base::InputState;
use crate::input::state::core::properties::utils::{cycle_index, palette_position, palette_step};
use crate::input::state::{Toast, ToastPriority};

/// The color `shape` has now and the one a recolor to `target` gives it, or
/// `None` for a shape without a color. A marker keeps its own opacity, which
/// is what makes it a highlighter; every other shape takes `target` whole,
/// opacity included.
fn recolored(shape: &Shape, target: Color) -> Option<(Color, Color)> {
    let current = match shape {
        Shape::Freehand { color, .. }
        | Shape::FreehandPressure { color, .. }
        | Shape::Line { color, .. }
        | Shape::Rect { color, .. }
        | Shape::Ellipse { color, .. }
        | Shape::Polygon { color, .. }
        | Shape::Arrow { color, .. }
        | Shape::MarkerStroke { color, .. }
        | Shape::Text { color, .. }
        | Shape::StepMarker { color, .. } => *color,
        Shape::StickyNote { background, .. } => *background,
        _ => return None,
    };
    let next = match shape {
        Shape::MarkerStroke { .. } => Color {
            a: current.a,
            ..target
        },
        _ => target,
    };
    Some((current, next))
}

fn shape_color_mut(shape: &mut Shape) -> Option<&mut Color> {
    match shape {
        Shape::Freehand { color, .. }
        | Shape::FreehandPressure { color, .. }
        | Shape::Line { color, .. }
        | Shape::Rect { color, .. }
        | Shape::Ellipse { color, .. }
        | Shape::Polygon { color, .. }
        | Shape::Arrow { color, .. }
        | Shape::MarkerStroke { color, .. }
        | Shape::Text { color, .. }
        | Shape::StepMarker { color, .. } => Some(color),
        Shape::StickyNote { background, .. } => Some(background),
        _ => None,
    }
}

impl InputState {
    pub(crate) fn apply_selection_color_value_with(
        &mut self,
        measurer: &TextMeasurer,
        target: Color,
    ) -> bool {
        let result = self.apply_selection_change_with(
            measurer,
            |shape| recolored(shape, target).is_some(),
            |shape| {
                let Some((current, next)) = recolored(shape, target) else {
                    return false;
                };
                if current == next {
                    return false;
                }
                let Some(color) = shape_color_mut(shape) else {
                    return false;
                };
                *color = next;
                true
            },
        );

        self.report_selection_apply_result(result, "color")
    }

    /// Whether recoloring the selection to `target` would change any shape it
    /// may edit, opacity included. Surfaces that set a color directly ask
    /// this first, so picking the color a selection already has is quiet.
    pub(crate) fn selection_recolor_changes(&self, target: Color) -> bool {
        let frame = self.boards.active_frame();
        self.selected_shape_ids()
            .iter()
            .filter_map(|id| frame.shape(*id))
            .filter(|drawn| !drawn.locked)
            .filter_map(|drawn| recolored(&drawn.shape, target))
            .any(|(current, next)| current != next)
    }

    /// Steps the selection's color through the quick-color palette, the same
    /// swatches the toolbar and the properties panel offer.
    pub(in crate::input::state::core::properties) fn apply_selection_color(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let palette: Vec<Color> = self
            .style
            .quick_colors
            .rendered_entries()
            .iter()
            .map(|entry| entry.color)
            .collect();
        let current = self
            .selection_primary_color()
            .and_then(|color| palette_position(palette.iter().copied(), color));
        let offset = if direction < 0 { -1 } else { 1 };
        let Some(first) = palette_step(palette.len(), current, offset) else {
            self.push_toast(
                ToastPriority::Info,
                "selection.apply",
                Toast::warning("The quick-color palette is empty."),
            );
            return false;
        };
        // Step past entries that would leave the selection as it is: a marker
        // keeps its own opacity, so the next swatch of the same hue changes
        // nothing, and stopping there would pin every later step to it.
        let mut next = first;
        for _ in 0..palette.len() {
            if self.selection_recolor_changes(palette[next]) {
                break;
            }
            next = cycle_index(next, palette.len(), offset);
        }
        let target = palette[next];

        self.apply_selection_color_value_with(measurer, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;
    use crate::draw::RED;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let _action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        crate::input::state::test_support::make_test_input_state()
    }

    #[test]
    fn apply_selection_color_value_preserves_marker_alpha() {
        let route_measurer = crate::draw::TextMeasurer::default();
        let mut state = make_state();
        let marker_id = state
            .boards
            .active_frame_mut()
            .add_shape(Shape::MarkerStroke {
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

        assert!(state.apply_selection_color_value_with(&route_measurer, RED));

        match &state
            .boards
            .active_frame()
            .shape(marker_id)
            .expect("marker")
            .shape
        {
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

    fn add_rect(state: &mut InputState, color: Color) -> crate::draw::ShapeId {
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            color,
            thick: 2.0,
        })
    }

    fn rect_color(state: &InputState, id: crate::draw::ShapeId) -> Color {
        match &state.boards.active_frame().shape(id).expect("rect").shape {
            Shape::Rect { color, .. } => *color,
            other => panic!("expected rect, got {other:?}"),
        }
    }

    #[test]
    fn apply_selection_color_wraps_the_quick_color_palette_forward() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let palette = state.style.quick_colors.rendered_entries().to_vec();
        let last = palette.last().expect("palette").color;
        let rect_id = add_rect(&mut state, last);
        state.set_selection(vec![rect_id]);

        assert!(state.apply_selection_color(&measurer, 0));

        assert_eq!(rect_color(&state, rect_id), palette[0].color);
    }

    #[test]
    fn apply_selection_color_steps_into_the_palette_from_a_custom_color() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let palette = state.style.quick_colors.rendered_entries().to_vec();
        let custom = Color {
            r: 0.13,
            g: 0.27,
            b: 0.61,
            a: 1.0,
        };
        let rect_id = add_rect(&mut state, custom);
        state.set_selection(vec![rect_id]);

        assert!(state.apply_selection_color(&measurer, 1));
        assert_eq!(
            rect_color(&state, rect_id),
            palette[0].color,
            "forward from outside the palette lands on its first swatch"
        );

        let rect_id = add_rect(&mut state, custom);
        state.set_selection(vec![rect_id]);
        assert!(state.apply_selection_color(&measurer, -1));
        assert_eq!(
            rect_color(&state, rect_id),
            palette[palette.len() - 1].color,
            "backward from outside the palette lands on its last swatch"
        );
    }
}
