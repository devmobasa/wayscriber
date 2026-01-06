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
        let frame = self.canvas_set.active_frame();
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
        let frame = self.canvas_set.active_frame();
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
            let frame = self.canvas_set.active_frame_mut();
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

        self.canvas_set
            .active_frame_mut()
            .push_undo_action(undo_action, self.undo_stack_limit);

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
