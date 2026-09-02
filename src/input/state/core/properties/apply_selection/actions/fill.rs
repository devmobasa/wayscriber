use crate::draw::Shape;
use crate::input::state::core::base::InputState;
use crate::input::state::{Toast, ToastPriority};

impl InputState {
    pub(in crate::input::state::core::properties) fn apply_selection_fill(
        &mut self,
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

        let result = self.apply_selection_change(
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
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        InputState::from_seed(crate::input::InputStateSeed {
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            thickness: 4.0,
            eraser_size: 4.0,
            eraser_mode: EraserMode::Brush,
            marker_opacity: 0.32,
            fill_enabled: false,
            font_size: 32.0,
            font_descriptor: FontDescriptor::default(),
            text_background_enabled: false,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            arrow_head_at_end: false,
            show_status_bar: true,
            boards_config: BoardsConfig::default(),
            action_map: action_map,
            max_shapes_per_frame: usize::MAX,
            click_highlight_settings: ClickHighlightSettings::disabled(),
            undo_all_delay_ms: 0,
            redo_all_delay_ms: 0,
            custom_section_enabled: true,
            custom_undo_delay_ms: 0,
            custom_redo_delay_ms: 0,
            custom_undo_steps: 5,
            custom_redo_steps: 5,
            presenter_mode_config: PresenterModeConfig::default(),
        })
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
