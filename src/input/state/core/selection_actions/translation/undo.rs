use crate::draw::ShapeId;
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::input::InputState;

impl InputState {
    pub(crate) fn push_translation_undo(&mut self, before: Vec<(ShapeId, ShapeSnapshot)>) -> bool {
        if before.is_empty() {
            return false;
        }

        let mut actions = Vec::new();
        {
            let frame = self.boards.active_frame();
            for (shape_id, before_snapshot) in &before {
                if let Some(shape) = frame.shape(*shape_id) {
                    let after_snapshot = ShapeSnapshot {
                        shape: shape.shape.clone(),
                        locked: shape.locked,
                    };
                    actions.push(UndoAction::Modify {
                        shape_id: *shape_id,
                        before: before_snapshot.clone(),
                        after: after_snapshot,
                    });
                }
            }
        }

        if actions.is_empty() {
            return false;
        }

        let undo_action = if actions.len() == 1 {
            actions.into_iter().next().unwrap()
        } else {
            UndoAction::Compound(actions)
        };

        self.boards
            .active_frame_mut()
            .push_undo_action(undo_action, self.undo_stack_limit);
        self.mark_session_dirty();
        true
    }
}
