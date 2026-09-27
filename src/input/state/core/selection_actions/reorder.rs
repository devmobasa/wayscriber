use super::super::base::InputState;
use crate::draw::TextMeasurer;
use crate::draw::frame::UndoAction;

impl InputState {
    pub(crate) fn move_selection_to_front_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.reorder_selection(measurer, true)
    }

    pub(crate) fn move_selection_to_back_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.reorder_selection(measurer, false)
    }

    /// Moves every selected shape one step up, past the nearest unselected
    /// shape above it. A run of selected shapes moves as a block, so their
    /// order among themselves stays as it was.
    pub(crate) fn move_selection_forward_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.step_selection(measurer, true)
    }

    /// Moves every selected shape one step down; see
    /// [`Self::move_selection_forward_with`].
    pub(crate) fn move_selection_backward_with(&mut self, measurer: &TextMeasurer) -> bool {
        self.step_selection(measurer, false)
    }

    /// Whether some selected shape has an unselected shape above it (or below
    /// it, for `forward == false`), so a step that way would move something.
    pub(crate) fn selection_can_step(&self, forward: bool) -> bool {
        let selected = self.selected_shape_ids();
        let shapes = &self.boards.active_frame().shapes;
        let is_selected = |index: usize| selected.contains(&shapes[index].id);
        (0..shapes.len()).any(|index| {
            is_selected(index)
                && if forward {
                    (index + 1..shapes.len()).any(|above| !is_selected(above))
                } else {
                    (0..index).any(|below| !is_selected(below))
                }
        })
    }

    fn step_selection(&mut self, measurer: &TextMeasurer, forward: bool) -> bool {
        let selected: Vec<_> = self.selected_shape_ids().to_vec();
        if selected.is_empty() {
            return false;
        }

        let len = self.boards.active_frame().shapes.len();
        // Walk against the direction of travel, so a shape that just moved is
        // never visited again and a selected block shifts as one.
        let indices: Vec<usize> = if forward {
            (0..len.saturating_sub(1)).rev().collect()
        } else {
            (1..len).collect()
        };
        let mut actions = Vec::new();
        for from in indices {
            let to = if forward { from + 1 } else { from - 1 };
            let (moving, neighbour) = {
                let shapes = &self.boards.active_frame().shapes;
                (shapes[from].id, shapes[to].id)
            };
            if !selected.contains(&moving) || selected.contains(&neighbour) {
                continue;
            }
            if self
                .boards
                .active_frame_mut()
                .move_shape(from, to)
                .is_some()
            {
                actions.push(UndoAction::Reorder {
                    shape_id: moving,
                    from,
                    to,
                });
                for id in [moving, neighbour] {
                    if let Some(shape) = self.boards.active_frame().shape(id) {
                        self.dirty_tracker.mark_shape_with(&shape.shape, measurer);
                    }
                    self.invalidate_hit_cache_for_with(measurer, id);
                }
            }
        }

        if actions.is_empty() {
            return false;
        }

        self.boards.active_frame_mut().push_undo_action(
            UndoAction::Compound { actions },
            self.history_limits.undo_stack_limit(),
        );
        self.mark_session_dirty();
        true
    }

    fn reorder_selection(&mut self, measurer: &TextMeasurer, to_front: bool) -> bool {
        let ids_len = self.selected_shape_ids().len();
        if ids_len == 0 {
            return false;
        }

        let mut actions = Vec::new();
        let len = self.boards.active_frame().shapes.len();
        for idx in 0..ids_len {
            let id = self.selected_shape_ids()[idx];
            let movement = {
                let frame = self.boards.active_frame_mut();
                if let Some(from) = frame.find_index(id) {
                    let target = if to_front { len.saturating_sub(1) } else { 0 };
                    if from == target {
                        None
                    } else if frame.move_shape(from, target).is_some() {
                        Some((from, target))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((from, target)) = movement {
                actions.push(UndoAction::Reorder {
                    shape_id: id,
                    from,
                    to: target,
                });
                if let Some(shape) = self.boards.active_frame().shape(id) {
                    self.dirty_tracker.mark_shape_with(&shape.shape, measurer);
                    self.invalidate_hit_cache_for_with(measurer, id);
                }
            }
        }

        if actions.is_empty() {
            return false;
        }

        self.boards.active_frame_mut().push_undo_action(
            UndoAction::Compound { actions },
            self.history_limits.undo_stack_limit(),
        );
        self.mark_session_dirty();
        true
    }
}
