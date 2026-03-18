use super::base::InputState;
use crate::draw::frame::UndoAction;

impl InputState {
    /// Applies side effects after an undoable action mutates the frame.
    pub fn apply_action_side_effects(&mut self, action: &UndoAction) {
        self.invalidate_hit_cache_from_action(action);
        self.mark_dirty_from_action(action);
        self.clear_selection();
        self.needs_redraw = true;
        self.mark_session_dirty();
    }

    fn mark_dirty_from_action(&mut self, action: &UndoAction) {
        if self.is_properties_panel_open() {
            self.properties_panel_needs_refresh = true;
        }
        match action {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.dirty_tracker.mark_shape(&shape.shape);
                }
            }
            UndoAction::Modify {
                before,
                after,
                shape_id,
                ..
            } => {
                self.dirty_tracker.mark_shape(&before.shape);
                self.dirty_tracker.mark_shape(&after.shape);
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Reorder { shape_id, .. } => {
                if let Some(shape) = self.boards.active_frame().shape(*shape_id) {
                    self.dirty_tracker.mark_shape(&shape.shape);
                    self.invalidate_hit_cache_for(*shape_id);
                }
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.mark_dirty_from_action(action);
                }
            }
        }
    }

    fn invalidate_hit_cache_from_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } | UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.invalidate_hit_cache_for(shape.id);
                }
            }
            UndoAction::Modify { shape_id, .. } => {
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Reorder { shape_id, .. } => {
                self.invalidate_hit_cache_for(*shape_id);
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.invalidate_hit_cache_from_action(action);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardsConfig, KeybindingsConfig, PresenterModeConfig};
    use crate::draw::{Color, FontDescriptor, Shape, frame::ShapeSnapshot};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        let mut state = InputState::with_defaults(
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
        );
        state.update_screen_dimensions(200, 120);
        let _ = state.take_dirty_regions();
        state
    }

    fn rect(x: i32, y: i32) -> Shape {
        Shape::Rect {
            x,
            y,
            w: 10,
            h: 12,
            fill: false,
            color: Color {
                r: 0.2,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            },
            thick: 2.0,
        }
    }

    #[test]
    fn apply_action_side_effects_clears_selection_and_sets_redraw_flags() {
        let mut state = make_state();
        let shape_id = state.boards.active_frame_mut().add_shape(rect(10, 20));
        let drawn = state
            .boards
            .active_frame()
            .shape(shape_id)
            .expect("shape")
            .clone();
        state.set_selection(vec![shape_id]);
        let _ = state.take_dirty_regions();
        state.needs_redraw = false;
        state.session_dirty = false;

        state.apply_action_side_effects(&UndoAction::Create {
            shapes: vec![(0, drawn)],
        });

        assert!(state.selected_shape_ids().is_empty());
        assert!(state.needs_redraw);
        assert!(state.session_dirty);
        assert!(!state.take_dirty_regions().is_empty());
    }

    #[test]
    fn apply_action_side_effects_closes_properties_panel_after_modify() {
        let mut state = make_state();
        let shape_id = state.boards.active_frame_mut().add_shape(rect(10, 20));
        state.set_selection(vec![shape_id]);
        assert!(state.show_properties_panel());
        assert!(state.is_properties_panel_open());
        let _ = state.take_dirty_regions();

        state.apply_action_side_effects(&UndoAction::Modify {
            shape_id,
            before: ShapeSnapshot {
                shape: rect(10, 20),
                locked: false,
            },
            after: ShapeSnapshot {
                shape: rect(30, 40),
                locked: false,
            },
        });

        assert!(!state.is_properties_panel_open());
        assert!(state.selected_shape_ids().is_empty());
        assert!(state.needs_redraw);
        assert!(!state.take_dirty_regions().is_empty());
    }

    #[test]
    fn apply_action_side_effects_marks_dirty_for_reorder_when_shape_still_exists() {
        let mut state = make_state();
        let shape_id = state.boards.active_frame_mut().add_shape(rect(5, 5));
        let _ = state.take_dirty_regions();
        state.needs_redraw = false;

        state.apply_action_side_effects(&UndoAction::Reorder {
            shape_id,
            from: 0,
            to: 1,
        });

        assert!(state.needs_redraw);
        assert!(!state.take_dirty_regions().is_empty());
    }

    #[test]
    fn apply_action_side_effects_marks_dirty_for_compound_actions() {
        let mut state = make_state();
        let first_id = state.boards.active_frame_mut().add_shape(rect(0, 0));
        let second_id = state.boards.active_frame_mut().add_shape(rect(40, 40));
        let first = state
            .boards
            .active_frame()
            .shape(first_id)
            .expect("first shape")
            .clone();
        let second = state
            .boards
            .active_frame()
            .shape(second_id)
            .expect("second shape")
            .clone();
        let _ = state.take_dirty_regions();

        state.apply_action_side_effects(&UndoAction::Compound(vec![
            UndoAction::Create {
                shapes: vec![(0, first)],
            },
            UndoAction::Delete {
                shapes: vec![(1, second)],
            },
        ]));

        assert!(state.session_dirty);
        assert!(!state.take_dirty_regions().is_empty());
    }
}
