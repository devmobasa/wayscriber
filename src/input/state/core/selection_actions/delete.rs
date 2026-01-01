use super::super::base::InputState;
use crate::draw::ShapeId;
use crate::draw::frame::UndoAction;
use std::collections::HashSet;

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
                    if shape.locked {
                        continue;
                    }
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
}

#[cfg(test)]
mod tests;
