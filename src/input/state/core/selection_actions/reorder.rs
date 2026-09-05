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
