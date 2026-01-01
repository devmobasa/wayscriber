use super::super::base::InputState;
use crate::draw::ShapeId;
use crate::draw::frame::UndoAction;

impl InputState {
    pub(crate) fn move_selection_to_front(&mut self) -> bool {
        self.reorder_selection(true)
    }

    pub(crate) fn move_selection_to_back(&mut self) -> bool {
        self.reorder_selection(false)
    }

    fn reorder_selection(&mut self, to_front: bool) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut actions = Vec::new();
        let len = self.canvas_set.active_frame().shapes.len();
        for id in ids {
            let movement = {
                let frame = self.canvas_set.active_frame_mut();
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
                if let Some(shape) = self.canvas_set.active_frame().shape(id) {
                    self.dirty_tracker.mark_shape(&shape.shape);
                    self.invalidate_hit_cache_for(id);
                }
            }
        }

        if actions.is_empty() {
            return false;
        }

        self.canvas_set
            .active_frame_mut()
            .push_undo_action(UndoAction::Compound(actions), self.undo_stack_limit);
        true
    }
}
