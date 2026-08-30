use super::super::super::core::Frame;
use super::super::super::types::{DrawnShape, ShapeId, UndoAction};

impl Frame {
    pub(super) fn apply_action(&mut self, action: &UndoAction) {
        match action {
            UndoAction::Create { shapes } => {
                self.restore_shapes_at_recorded_indices(shapes);
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
                    target.set_shape(after.shape.clone());
                    target.locked = after.locked;
                }
            }
            UndoAction::ModifyImageBounds {
                shape_id, after, ..
            } => {
                self.apply_image_bounds(*shape_id, *after);
            }
            UndoAction::Reorder {
                shape_id,
                from: _,
                to,
            } => {
                self.move_shape_to(*shape_id, *to);
            }
            UndoAction::Compound { actions } => {
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
                self.restore_shapes_at_recorded_indices(shapes);
            }
            UndoAction::Modify {
                shape_id, before, ..
            } => {
                if let Some(target) = self.shape_mut(*shape_id) {
                    target.set_shape(before.shape.clone());
                    target.locked = before.locked;
                }
            }
            UndoAction::ModifyImageBounds {
                shape_id, before, ..
            } => {
                self.apply_image_bounds(*shape_id, *before);
            }
            UndoAction::Reorder { shape_id, from, .. } => {
                self.move_shape_to(*shape_id, *from);
            }
            UndoAction::Compound { actions } => {
                for action in actions.iter().rev() {
                    self.apply_inverse(action);
                }
            }
        }
    }

    fn restore_shapes_at_recorded_indices(&mut self, shapes: &[(usize, DrawnShape)]) {
        let mut items: Vec<_> = shapes.iter().collect();
        items.sort_by_key(|(index, _)| *index);
        for (index, shape) in items {
            self.insert_existing((*index).min(self.shapes.len()), shape.clone());
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
            self.bump_shape_order_generation();
        }
    }

    fn apply_image_bounds(
        &mut self,
        shape_id: ShapeId,
        bounds: super::super::super::types::ImageBoundsSnapshot,
    ) {
        if let Some(target) = self.shape_mut(shape_id)
            && let crate::draw::shape::Shape::Image { x, y, w, h, .. } = &mut target.shape
        {
            *x = bounds.x;
            *y = bounds.y;
            *w = bounds.w;
            *h = bounds.h;
            target.locked = bounds.locked;
            target.invalidate_bounds();
        }
    }
}
