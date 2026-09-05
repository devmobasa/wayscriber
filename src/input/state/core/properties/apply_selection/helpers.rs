use super::super::super::base::InputState;
use super::super::summary::shape_color;
use crate::draw::{Color, Shape, TextMeasurer};
use crate::input::state::{Toast, ToastPriority};

#[derive(Default)]
pub(in crate::input::state::core) struct SelectionApplyResult {
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

    pub(in crate::input::state::core) fn apply_selection_change_with<A, F>(
        &mut self,
        measurer: &TextMeasurer,
        applicable: A,
        apply: F,
    ) -> SelectionApplyResult
    where
        A: FnMut(&Shape) -> bool,
        F: FnMut(&mut Shape) -> bool,
    {
        let ids = self.selected_shape_ids().to_vec();
        let (changed, locked, applicable, effects) =
            crate::input::state::core::editing::CanvasEdit::apply_selection(
                self.boards.active_frame_mut(),
                &ids,
                measurer,
                self.history_limits.undo_stack_limit(),
                applicable,
                apply,
            );
        self.apply_edit_effects(measurer, effects);
        SelectionApplyResult {
            changed,
            locked,
            applicable,
        }
    }

    pub(in crate::input::state::core) fn report_selection_apply_result(
        &mut self,
        result: SelectionApplyResult,
        label: &str,
    ) -> bool {
        if result.applicable == 0 {
            self.push_toast(
                ToastPriority::Info,
                "selection.apply",
                Toast::warning(format!("No {label} to edit in selection.")),
            );
            return false;
        }

        if result.changed == 0 {
            if result.locked == result.applicable {
                self.push_toast(
                    ToastPriority::Info,
                    "selection.apply",
                    Toast::warning(format!("All {label} shapes are locked.")),
                );
            } else {
                self.push_toast(
                    ToastPriority::Info,
                    "selection.apply",
                    Toast::info("No changes applied."),
                );
            }
            return false;
        }

        if result.locked > 0 {
            self.push_toast(
                ToastPriority::Info,
                "selection.apply",
                Toast::warning(format!("{} locked shape(s) unchanged.", result.locked)),
            );
        }
        true
    }
}

#[cfg(test)]
mod tests;
