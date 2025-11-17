use super::base::InputState;
use crate::draw::frame::UndoAction;

impl InputState {
    /// Applies side effects after an undoable action mutates the frame.
    pub fn apply_action_side_effects(&mut self, action: &UndoAction) {
        self.invalidate_hit_cache_from_action(action);
        self.mark_dirty_from_action(action);
        self.clear_selection();
        self.needs_redraw = true;
    }

    fn mark_dirty_from_action(&mut self, action: &UndoAction) {
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
                if let Some(shape) = self.canvas_set.active_frame().shape(*shape_id) {
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
