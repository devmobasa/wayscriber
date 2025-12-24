use super::base::{DrawingState, InputState};
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{DrawnShape, Shape, ShapeId};
use crate::util::Rect;
use std::collections::HashSet;

const SELECTION_HALO_PADDING: i32 = 6;

impl InputState {
    pub(crate) fn delete_selection(&mut self) -> bool {
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        self.delete_shapes_by_ids(&ids)
    }

    pub(crate) fn delete_shapes_by_ids(&mut self, ids: &[ShapeId]) -> bool {
        if ids.is_empty() {
            return false;
        }

        let id_set: HashSet<ShapeId> = ids.iter().copied().collect();
        if id_set.is_empty() {
            return false;
        }

        let mut removed = Vec::new();
        let mut dirty = Vec::new();
        {
            let frame = self.canvas_set.active_frame();
            for (index, shape) in frame.shapes.iter().enumerate() {
                if id_set.contains(&shape.id) {
                    dirty.push((shape.id, shape.shape.bounding_box()));
                    removed.push((index, shape.clone()));
                }
            }
        }

        if removed.is_empty() {
            return false;
        }

        {
            let frame = self.canvas_set.active_frame_mut();
            for (index, _) in removed.iter().rev() {
                frame.shapes.remove(*index);
            }
            frame.push_undo_action(
                UndoAction::Delete { shapes: removed },
                self.undo_stack_limit,
            );
        }

        for (shape_id, bounds) in dirty {
            self.mark_selection_dirty_region(bounds);
            self.invalidate_hit_cache_for(shape_id);
        }

        self.clear_selection();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn erase_strokes_by_points(&mut self, points: &[(i32, i32)]) -> bool {
        let sampled = self.sample_eraser_path_points(points);
        let ids = self.hit_test_all_for_points(&sampled, self.eraser_hit_radius());
        self.delete_shapes_by_ids(&ids)
    }

    fn sample_eraser_path_points(&self, points: &[(i32, i32)]) -> Vec<(i32, i32)> {
        if points.len() < 2 {
            return points.to_vec();
        }

        let step = (self.eraser_hit_radius() * 0.9).max(1.0);
        let mut needs_sampling = false;
        for window in points.windows(2) {
            let dx = (window[1].0 - window[0].0) as f64;
            let dy = (window[1].1 - window[0].1) as f64;
            if (dx * dx + dy * dy).sqrt() > step {
                needs_sampling = true;
                break;
            }
        }

        if !needs_sampling {
            return points.to_vec();
        }

        let mut sampled = Vec::with_capacity(points.len());
        sampled.push(points[0]);
        for window in points.windows(2) {
            let (x0, y0) = window[0];
            let (x1, y1) = window[1];
            let dx = (x1 - x0) as f64;
            let dy = (y1 - y0) as f64;
            let dist = (dx * dx + dy * dy).sqrt();
            let steps = ((dist / step).ceil() as i32).max(1);
            for i in 1..=steps {
                let t = i as f64 / steps as f64;
                let point = (
                    (x0 as f64 + dx * t).round() as i32,
                    (y0 as f64 + dy * t).round() as i32,
                );
                if sampled.last().copied() != Some(point) {
                    sampled.push(point);
                }
            }
        }
        sampled
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
            Self::translate_shape(&mut cloned_shape, 12, 12);
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
                self.mark_selection_dirty_region(stored.shape.bounding_box());
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
        self.needs_redraw = true;
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

    pub(crate) fn capture_movable_selection_snapshots(&self) -> Vec<(ShapeId, ShapeSnapshot)> {
        let frame = self.canvas_set.active_frame();
        self.selected_shape_ids()
            .iter()
            .filter_map(|id| {
                frame.shape(*id).and_then(|shape| {
                    if shape.locked {
                        None
                    } else {
                        Some((
                            *id,
                            ShapeSnapshot {
                                shape: shape.shape.clone(),
                                locked: shape.locked,
                            },
                        ))
                    }
                })
            })
            .collect()
    }

    pub(crate) fn apply_translation_to_selection(&mut self, dx: i32, dy: i32) -> bool {
        if dx == 0 && dy == 0 {
            return false;
        }
        let ids: Vec<ShapeId> = self.selected_shape_ids().to_vec();
        if ids.is_empty() {
            return false;
        }

        let mut moved_any = false;
        for id in ids {
            let bounds = {
                let frame = self.canvas_set.active_frame_mut();
                if let Some(shape) = frame.shape_mut(id) {
                    if shape.locked {
                        None
                    } else {
                        let before = shape.shape.bounding_box();
                        Self::translate_shape(&mut shape.shape, dx, dy);
                        let after = shape.shape.bounding_box();
                        Some((before, after))
                    }
                } else {
                    None
                }
            };

            if let Some((before_bounds, after_bounds)) = bounds {
                self.mark_selection_dirty_region(before_bounds);
                self.mark_selection_dirty_region(after_bounds);
                self.invalidate_hit_cache_for(id);
                moved_any = true;
            }
        }

        if moved_any {
            self.needs_redraw = true;
        }
        moved_any
    }

    pub(crate) fn push_translation_undo(&mut self, before: Vec<(ShapeId, ShapeSnapshot)>) -> bool {
        if before.is_empty() {
            return false;
        }

        let mut actions = Vec::new();
        {
            let frame = self.canvas_set.active_frame();
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

        self.canvas_set
            .active_frame_mut()
            .push_undo_action(undo_action, self.undo_stack_limit);
        true
    }

    pub(crate) fn translate_selection_with_undo(&mut self, dx: i32, dy: i32) -> bool {
        if dx == 0 && dy == 0 {
            return false;
        }
        let before = self.capture_movable_selection_snapshots();
        if before.is_empty() {
            return false;
        }
        if !self.apply_translation_to_selection(dx, dy) {
            return false;
        }
        self.push_translation_undo(before);
        true
    }

    pub(crate) fn restore_selection_from_snapshots(
        &mut self,
        snapshots: Vec<(ShapeId, ShapeSnapshot)>,
    ) {
        if snapshots.is_empty() {
            return;
        }

        for (shape_id, snapshot) in snapshots {
            let bounds = {
                let frame = self.canvas_set.active_frame_mut();
                if let Some(shape) = frame.shape_mut(shape_id) {
                    let before = shape.shape.bounding_box();
                    shape.shape = snapshot.shape.clone();
                    shape.locked = snapshot.locked;
                    let after = shape.shape.bounding_box();
                    Some((before, after))
                } else {
                    None
                }
            };
            if let Some((before_bounds, after_bounds)) = bounds {
                self.mark_selection_dirty_region(before_bounds);
                self.mark_selection_dirty_region(after_bounds);
                self.invalidate_hit_cache_for(shape_id);
            }
        }
        self.needs_redraw = true;
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
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
        true
    }

    pub(crate) fn edit_selected_text(&mut self) -> bool {
        if self.selected_shape_ids().len() != 1 {
            return false;
        }
        let shape_id = self.selected_shape_ids()[0];
        let frame = self.canvas_set.active_frame();
        if let Some(shape) = frame.shape(shape_id)
            && let Shape::Text { x, y, text, .. } = &shape.shape
        {
            self.state = DrawingState::TextInput {
                x: *x,
                y: *y,
                buffer: text.clone(),
            };
            self.update_text_preview_dirty();
            return true;
        }
        false
    }

    fn translate_shape(shape: &mut Shape, dx: i32, dy: i32) {
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
            Shape::MarkerStroke { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
            Shape::EraserStroke { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
        }
    }

    pub(crate) fn mark_selection_dirty_region(&mut self, rect: Option<Rect>) {
        if let Some(rect) = rect {
            if let Some(inflated) = rect.inflated(SELECTION_HALO_PADDING) {
                self.dirty_tracker.mark_rect(inflated);
            } else {
                self.dirty_tracker.mark_rect(rect);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BoardConfig, KeybindingsConfig};
    use crate::draw::{Color, FontDescriptor};
    use crate::input::{ClickHighlightSettings, EraserMode};

    fn create_test_input_state() -> InputState {
        let action_map = KeybindingsConfig::default()
            .build_action_map()
            .expect("action map");

        InputState::with_defaults(
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            3.0,
            12.0,
            EraserMode::Brush,
            0.32,
            false,
            32.0,
            FontDescriptor {
                family: "Sans".to_string(),
                weight: "bold".to_string(),
                style: "normal".to_string(),
            },
            false,
            20.0,
            30.0,
            false,
            true,
            BoardConfig::default(),
            action_map,
            usize::MAX,
            ClickHighlightSettings::disabled(),
            0,
            0,
            false,
            0,
            0,
            5,
            5,
        )
    }

    #[test]
    fn sample_eraser_path_points_densifies_long_segments() {
        let state = create_test_input_state();
        let points = vec![(0, 0), (20, 0)];
        let sampled = state.sample_eraser_path_points(&points);

        assert!(sampled.len() > points.len());
        assert_eq!(sampled.first().copied(), Some((0, 0)));
        assert_eq!(sampled.last().copied(), Some((20, 0)));
    }
}
