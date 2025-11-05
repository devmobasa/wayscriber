use super::base::{DrawingState, InputState};
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{DrawnShape, Shape, ShapeId};

impl InputState {
    pub(crate) fn delete_selection(&mut self) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut removed = Vec::new();
        for id in ids {
            if let Some((index, shape)) = self.canvas_set.active_frame_mut().remove_shape_by_id(id)
            {
                removed.push((index, shape));
            }
        }

        if removed.is_empty() {
            return false;
        }

        for (_, shape) in &removed {
            self.dirty_tracker.mark_shape(&shape.shape);
            self.invalidate_hit_cache_for(shape.id);
        }

        self.canvas_set.active_frame_mut().push_undo_action(
            UndoAction::Delete { shapes: removed },
            self.undo_stack_limit,
        );
        self.clear_selection();
        true
    }

    pub(crate) fn duplicate_selection(&mut self) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut created = Vec::new();
        let mut new_ids = Vec::new();
        for id in ids {
            let original = {
                let frame = self.canvas_set.active_frame();
                frame.shape(id).cloned()
            };
            let Some(shape) = original else {
                continue;
            };
            if shape.locked {
                continue;
            }

            let mut cloned_shape = shape.shape.clone();
            Self::offset_shape(&mut cloned_shape, 12, 12);
            let new_id = {
                let frame = self.canvas_set.active_frame_mut();
                frame.add_shape(cloned_shape)
            };

            if let Some((index, stored)) = {
                let frame = self.canvas_set.active_frame();
                frame
                    .find_index(new_id)
                    .and_then(|idx| frame.shape(new_id).map(|s| (idx, s.clone())))
            } {
                self.dirty_tracker.mark_shape(&stored.shape);
                self.invalidate_hit_cache_for(new_id);
                created.push((index, stored));
                new_ids.push(new_id);
            }
        }

        if created.is_empty() {
            return false;
        }

        self.canvas_set.active_frame_mut().push_undo_action(
            UndoAction::Create { shapes: created },
            self.undo_stack_limit,
        );
        self.set_selection(new_ids);
        true
    }

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

    pub(crate) fn set_selection_locked(&mut self, locked: bool) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut actions = Vec::new();
        for id in ids {
            let result = {
                let frame = self.canvas_set.active_frame_mut();
                if let Some(shape) = frame.shape_mut(id) {
                    if shape.locked == locked {
                        None
                    } else {
                        let before = ShapeSnapshot {
                            shape: shape.shape.clone(),
                            locked: !locked,
                        };
                        shape.locked = locked;
                        let after = ShapeSnapshot {
                            shape: shape.shape.clone(),
                            locked,
                        };
                        Some((before, after, shape.shape.clone()))
                    }
                } else {
                    None
                }
            };

            if let Some((before, after, shape_for_dirty)) = result {
                actions.push(UndoAction::Modify {
                    shape_id: id,
                    before,
                    after,
                });
                self.dirty_tracker.mark_shape(&shape_for_dirty);
                self.invalidate_hit_cache_for(id);
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

    pub(crate) fn clear_all(&mut self) -> bool {
        let frame = self.canvas_set.active_frame_mut();
        if frame.shapes.is_empty() {
            return false;
        }

        let removed: Vec<(usize, DrawnShape)> = frame.shapes.iter().cloned().enumerate().collect();
        frame.shapes.clear();
        frame.push_undo_action(
            UndoAction::Delete { shapes: removed },
            self.undo_stack_limit,
        );
        self.invalidate_hit_cache();
        self.clear_selection();
        true
    }

    pub(crate) fn edit_selected_text(&mut self) -> bool {
        if self.selected_shape_ids().len() != 1 {
            return false;
        }
        let shape_id = self.selected_shape_ids()[0];
        let frame = self.canvas_set.active_frame();
        if let Some(shape) = frame.shape(shape_id) {
            if let Shape::Text { x, y, text, .. } = &shape.shape {
                self.state = DrawingState::TextInput {
                    x: *x,
                    y: *y,
                    buffer: text.clone(),
                };
                self.update_text_preview_dirty();
                return true;
            }
        }
        false
    }

    fn offset_shape(shape: &mut Shape, dx: i32, dy: i32) {
        match shape {
            Shape::Freehand { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
            Shape::Line { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Shape::Rect { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::Ellipse { cx, cy, .. } => {
                *cx += dx;
                *cy += dy;
            }
            Shape::Arrow { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Shape::Text { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
        }
    }
}
