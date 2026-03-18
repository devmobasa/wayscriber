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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
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
    fn apply_selection_fill_on_mixed_selection_turns_all_fills_on() {
        let mut state = make_state();
        let rect_id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
            fill: false,
            color: state.current_color,
            thick: 2.0,
        });
        let ellipse_id = state.boards.active_frame_mut().add_shape(Shape::Ellipse {
            cx: 26,
            cy: 27,
            rx: 6,
            ry: 7,
            fill: true,
            color: state.current_color,
            thick: 2.0,
        });
        state.set_selection(vec![rect_id, ellipse_id]);

        assert!(state.apply_selection_fill(0));

        match &state.boards.active_frame().shape(rect_id).expect("rect").shape {
            Shape::Rect { fill, .. } => assert!(*fill),
            other => panic!("expected rect, got {other:?}"),
        }
        match &state.boards.active_frame().shape(ellipse_id).expect("ellipse").shape {
            Shape::Ellipse { fill, .. } => assert!(*fill),
            other => panic!("expected ellipse, got {other:?}"),
        }
    }
}
