use crate::draw::Shape;
use crate::draw::TextMeasurer;
use crate::input::state::core::base::InputState;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_fill(
        &mut self,
        measurer: &TextMeasurer,
        direction: i32,
    ) -> bool {
        let target = if direction == 0 {
            self.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. }
                | Shape::Ellipse { fill, .. }
                | Shape::Polygon { fill, .. } => Some(*fill),
                _ => None,
            })
        } else {
            Some(direction > 0)
        };

        let Some(target) = target else {
            self.push_toast(
                ToastPriority::Info,
                "selection.apply",
                Toast::warning("No fill shapes selected."),
            );
            return false;
        };

        let result = self.apply_selection_change_with(
            measurer,
            |shape| {
                matches!(
                    shape,
                    Shape::Rect { .. } | Shape::Ellipse { .. } | Shape::Polygon { .. }
                )
            },
            |shape| match shape {
                Shape::Rect { fill, .. }
                | Shape::Ellipse { fill, .. }
                | Shape::Polygon { fill, .. }
                    if *fill != target =>
                {
                    *fill = target;
                    true
                }
                _ => false,
            },
        );

        self.report_selection_apply_result(result, "fill")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let _action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        crate::input::state::test_support::make_test_input_state()
    }

    #[test]
    fn apply_selection_fill_on_mixed_selection_turns_all_fills_on() {
        let measurer = TextMeasurer::default();
        let mut state = make_state();
        let rect_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            color: state.style.current_color,
            thick: 2.0,
        });
        let ellipse_id = state.boards.active_frame_mut().add_shape(Shape::Ellipse {
            cx: 26,
            cy: 27,
            rx: 6,
            ry: 7,
            fill: true,
            color: state.style.current_color,
            thick: 2.0,
        });
        state.set_selection(vec![rect_id, ellipse_id]);

        assert!(state.apply_selection_fill(&measurer, 0));

        match &state
            .boards
            .active_frame()
            .shape(rect_id)
            .expect("rect")
            .shape
        {
            Shape::Rect { fill, .. } => assert!(*fill),
            other => panic!("expected rect, got {other:?}"),
        }
        match &state
            .boards
            .active_frame()
            .shape(ellipse_id)
            .expect("ellipse")
            .shape
        {
            Shape::Ellipse { fill, .. } => assert!(*fill),
            other => panic!("expected ellipse, got {other:?}"),
        }
    }
}
