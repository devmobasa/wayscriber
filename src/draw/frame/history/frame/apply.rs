use super::super::super::core::Frame;
use super::super::super::types::{ShapeId, UndoAction};

impl Frame {
    pub(super) fn apply_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } => {
                for (offset, (index, shape)) in shapes.iter().enumerate() {
                    self.insert_existing(index + offset, shape.clone());
                }
            }
            UndoAction::Delete { shapes } => {
                for (_, shape) in shapes {
                    self.remove_shape_by_id(shape.id);
                }
            }
            UndoAction::Modify {
                shape_id, after, ..
            } => {
                if let Some(target) = self.shape_mut(*shape_id) {
                    target.shape = after.shape.clone();
                    target.locked = after.locked;
                }
            }
            UndoAction::Reorder {
                shape_id,
                from: _,
                to,
            } => {
                self.move_shape_to(*shape_id, *to);
            }
            UndoAction::Compound(actions) => {
                for action in actions {
                    self.apply_action(action);
                }
            }
        }
    }

    pub(super) fn apply_inverse(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } => {
                for (_, shape) in shapes.iter().rev() {
                    self.remove_shape_by_id(shape.id);
                }
            }
            UndoAction::Delete { shapes } => {
                for (offset, (index, shape)) in shapes.iter().enumerate() {
                    let insert_at = (index + offset).min(self.shapes.len());
                    self.insert_existing(insert_at, shape.clone());
                }
            }
            UndoAction::Modify {
                shape_id, before, ..
            } => {
                if let Some(target) = self.shape_mut(*shape_id) {
                    target.shape = before.shape.clone();
                    target.locked = before.locked;
                }
            }
            UndoAction::Reorder { shape_id, from, .. } => {
                self.move_shape_to(*shape_id, *from);
            }
            UndoAction::Compound(actions) => {
                for action in actions.iter().rev() {
                    self.apply_inverse(action);
                }
            }
        }
    }

    fn move_shape_to(&mut self, shape_id: ShapeId, target: usize) {
        if let Some(index) = self.find_index(shape_id) {
            if index == target {
                return;
            }
            let shape = self.shapes.remove(index);
            let mut insert_index = target.min(self.shapes.len());
            if index < insert_index && insert_index > 0 {
                insert_index -= 1;
            }
            self.shapes.insert(insert_index, shape);
        }
    }
}
