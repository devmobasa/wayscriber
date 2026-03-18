use super::super::super::base::{InputState, UiToastKind};
use super::super::summary::shape_color;
use crate::draw::{Color, Shape};

#[derive(Default)]
pub(super) struct SelectionApplyResult {
    pub(super) changed: usize,
    pub(super) locked: usize,
    pub(super) applicable: usize,
}

impl InputState {
    pub(super) fn selection_primary_color(&self) -> Option<Color> {
        let frame = self.boards.active_frame();
        for id in self.selected_shape_ids() {
            let Some(drawn) = frame.shape(*id) else {
                continue;
            };
            if drawn.locked {
                continue;
            }
            if let Some(color) = shape_color(&drawn.shape) {
                return Some(color);
            }
        }
        None
    }

    pub(super) fn selection_bool_target<F>(&self, mut extract: F) -> Option<bool>
    where
        F: FnMut(&Shape) -> Option<bool>,
    {
        let frame = self.boards.active_frame();
        let mut applicable = 0;
        let mut editable_values = Vec::new();
        for id in self.selected_shape_ids() {
            if let Some(drawn) = frame.shape(*id)
                && let Some(value) = extract(&drawn.shape)
            {
                applicable += 1;
                if !drawn.locked {
                    editable_values.push(value);
                }
            }
        }
        if applicable == 0 {
            return None;
        }
        if editable_values.is_empty() {
            return Some(true);
        }
        let first = editable_values[0];
        let mixed = editable_values.iter().any(|v| *v != first);
        if mixed { Some(true) } else { Some(!first) }
    }

    pub(super) fn apply_selection_change<A, F>(
        &mut self,
        mut applicable: A,
        mut apply: F,
    ) -> SelectionApplyResult
    where
        A: FnMut(&Shape) -> bool,
        F: FnMut(&mut Shape) -> bool,
    {
        let ids_len = self.selected_shape_ids().len();
        if ids_len == 0 {
            return SelectionApplyResult::default();
        }

        let mut result = SelectionApplyResult::default();
        let mut actions = Vec::new();
        let mut dirty_regions = Vec::new();

        for idx in 0..ids_len {
            let id = self.selected_shape_ids()[idx];
            let frame = self.boards.active_frame_mut();
            let Some(drawn) = frame.shape_mut(id) else {
                continue;
            };
            if !applicable(&drawn.shape) {
                continue;
            }
            result.applicable += 1;
            if drawn.locked {
                result.locked += 1;
                continue;
            }

            let before_bounds = drawn.shape.bounding_box();
            let before_snapshot = crate::draw::frame::ShapeSnapshot {
                shape: drawn.shape.clone(),
                locked: drawn.locked,
            };

            let changed = apply(&mut drawn.shape);
            if !changed {
                continue;
            }

            let after_bounds = drawn.shape.bounding_box();
            let after_snapshot = crate::draw::frame::ShapeSnapshot {
                shape: drawn.shape.clone(),
                locked: drawn.locked,
            };

            actions.push(crate::draw::frame::UndoAction::Modify {
                shape_id: drawn.id,
                before: before_snapshot,
                after: after_snapshot,
            });
            dirty_regions.push((drawn.id, before_bounds, after_bounds));
            result.changed += 1;
        }

        if actions.is_empty() {
            return result;
        }

        let undo_action = if actions.len() == 1 {
            actions.into_iter().next().unwrap()
        } else {
            crate::draw::frame::UndoAction::Compound(actions)
        };

        self.boards
            .active_frame_mut()
            .push_undo_action(undo_action, self.undo_stack_limit);
        self.mark_session_dirty();

        for (shape_id, before, after) in dirty_regions {
            self.mark_selection_dirty_region(before);
            self.mark_selection_dirty_region(after);
            self.invalidate_hit_cache_for(shape_id);
        }
        self.needs_redraw = true;

        result
    }

    pub(super) fn report_selection_apply_result(
        &mut self,
        result: SelectionApplyResult,
        label: &str,
    ) -> bool {
        if result.applicable == 0 {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!("No {label} to edit in selection."),
            );
            return false;
        }

        if result.changed == 0 {
            if result.locked == result.applicable {
                self.set_ui_toast(
                    UiToastKind::Warning,
                    format!("All {label} shapes are locked."),
                );
            } else {
                self.set_ui_toast(UiToastKind::Info, "No changes applied.");
            }
            return false;
        }

        if result.locked > 0 {
            self.set_ui_toast(
                UiToastKind::Warning,
                format!("{} locked shape(s) unchanged.", result.locked),
            );
        }
        true
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

    fn add_rect(state: &mut InputState, color: Color, fill: bool, locked: bool) -> crate::draw::ShapeId {
        let id = state.boards.active_frame_mut().add_shape(Shape::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            fill,
            color,
            thick: 2.0,
        });
        if locked {
            let index = state.boards.active_frame().find_index(id).expect("shape index");
            state.boards.active_frame_mut().shapes[index].locked = true;
        }
        id
    }

    #[test]
    fn selection_primary_color_skips_locked_shapes() {
        let mut state = make_state();
        let locked = add_rect(
            &mut state,
            Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 },
            false,
            true,
        );
        let unlocked = add_rect(
            &mut state,
            Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
            false,
            false,
        );
        state.set_selection(vec![locked, unlocked]);

        assert_eq!(
            state.selection_primary_color(),
            Some(Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 })
        );
    }

    #[test]
    fn selection_bool_target_returns_true_for_mixed_or_locked_only_values() {
        let mut state = make_state();
        let first = add_rect(&mut state, Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, false, false);
        let second = add_rect(&mut state, Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 }, true, false);
        state.set_selection(vec![first, second]);

        assert_eq!(
            state.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. } => Some(*fill),
                _ => None,
            }),
            Some(true)
        );

        let mut locked_state = make_state();
        let locked = add_rect(
            &mut locked_state,
            Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
            false,
            true,
        );
        locked_state.set_selection(vec![locked]);
        assert_eq!(
            locked_state.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. } => Some(*fill),
                _ => None,
            }),
            Some(true)
        );
    }

    #[test]
    fn selection_bool_target_flips_uniform_unlocked_value() {
        let mut state = make_state();
        let first = add_rect(&mut state, Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }, false, false);
        let second = add_rect(&mut state, Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 }, false, false);
        state.set_selection(vec![first, second]);

        assert_eq!(
            state.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. } => Some(*fill),
                _ => None,
            }),
            Some(true)
        );

        let frame = state.boards.active_frame_mut();
        if let Shape::Rect { fill, .. } = &mut frame.shape_mut(first).expect("first shape").shape {
            *fill = true;
        }
        if let Shape::Rect { fill, .. } = &mut frame.shape_mut(second).expect("second shape").shape {
            *fill = true;
        }

        assert_eq!(
            state.selection_bool_target(|shape| match shape {
                Shape::Rect { fill, .. } => Some(*fill),
                _ => None,
            }),
            Some(false)
        );
    }

    #[test]
    fn apply_selection_change_reports_applicable_locked_and_changed_counts() {
        let mut state = make_state();
        let unlocked = add_rect(
            &mut state,
            Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
            false,
            false,
        );
        let locked = add_rect(
            &mut state,
            Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 },
            false,
            true,
        );
        state.set_selection(vec![unlocked, locked]);
        state.needs_redraw = false;
        state.session_dirty = false;

        let result = state.apply_selection_change(
            |shape| matches!(shape, Shape::Rect { .. }),
            |shape| match shape {
                Shape::Rect { fill, .. } => {
                    *fill = true;
                    true
                }
                _ => false,
            },
        );

        assert_eq!(result.applicable, 2);
        assert_eq!(result.locked, 1);
        assert_eq!(result.changed, 1);
        assert!(state.needs_redraw);
        assert!(state.session_dirty);
        assert_eq!(state.boards.active_frame().undo_stack_len(), 1);
        assert!(!state.take_dirty_regions().is_empty());
    }

    #[test]
    fn report_selection_apply_result_emits_expected_toasts() {
        let mut state = make_state();

        assert!(!state.report_selection_apply_result(
            SelectionApplyResult {
                changed: 0,
                locked: 0,
                applicable: 0,
            },
            "fill",
        ));
        assert_eq!(
            state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
            Some("No fill to edit in selection.")
        );

        assert!(!state.report_selection_apply_result(
            SelectionApplyResult {
                changed: 0,
                locked: 2,
                applicable: 2,
            },
            "color",
        ));
        assert_eq!(
            state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
            Some("All color shapes are locked.")
        );

        assert!(!state.report_selection_apply_result(
            SelectionApplyResult {
                changed: 0,
                locked: 1,
                applicable: 2,
            },
            "fill",
        ));
        assert_eq!(
            state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
            Some("No changes applied.")
        );

        assert!(state.report_selection_apply_result(
            SelectionApplyResult {
                changed: 1,
                locked: 2,
                applicable: 3,
            },
            "fill",
        ));
        assert_eq!(
            state.ui_toast.as_ref().map(|toast| toast.message.as_str()),
            Some("2 locked shape(s) unchanged.")
        );
    }
}
