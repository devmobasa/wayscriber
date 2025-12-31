use super::super::base::InputState;
use crate::draw::frame::{ShapeSnapshot, UndoAction};
use crate::draw::{Shape, ShapeId};
use crate::util::Rect;

impl InputState {
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
        let (dx, dy) = match self.clamp_selection_translation(dx, dy) {
            Some((dx, dy)) => (dx, dy),
            None => return false,
        };
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

    fn movable_selection_bounds(&self) -> Option<Rect> {
        let ids = self.selected_shape_ids();
        if ids.is_empty() {
            return None;
        }

        let frame = self.canvas_set.active_frame();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        let mut found = false;

        for id in ids {
            if let Some(shape) = frame.shape(*id) {
                if shape.locked {
                    continue;
                }
                if let Some(bounds) = shape.shape.bounding_box() {
                    min_x = min_x.min(bounds.x);
                    min_y = min_y.min(bounds.y);
                    max_x = max_x.max(bounds.x + bounds.width);
                    max_y = max_y.max(bounds.y + bounds.height);
                    found = true;
                }
            }
        }

        if found {
            Rect::from_min_max(min_x, min_y, max_x, max_y)
        } else {
            None
        }
    }

    fn clamp_axis_delta(position: i32, size: i32, screen: i32, delta: i32) -> i32 {
        if screen <= 0 || size <= 0 {
            return delta;
        }

        let end = position.saturating_add(size);
        let (min_delta, max_delta) = if size <= screen {
            (0i32.saturating_sub(position), screen.saturating_sub(end))
        } else {
            (screen.saturating_sub(end), 0i32.saturating_sub(position))
        };

        delta.clamp(min_delta, max_delta)
    }

    fn clamp_selection_translation(&self, dx: i32, dy: i32) -> Option<(i32, i32)> {
        let bounds = self.movable_selection_bounds()?;
        let screen_width = self.screen_width.min(i32::MAX as u32) as i32;
        let screen_height = self.screen_height.min(i32::MAX as u32) as i32;

        let clamped_dx = if dx == 0 {
            0
        } else {
            Self::clamp_axis_delta(bounds.x, bounds.width, screen_width, dx)
        };
        let clamped_dy = if dy == 0 {
            0
        } else {
            Self::clamp_axis_delta(bounds.y, bounds.height, screen_height, dy)
        };

        Some((clamped_dx, clamped_dy))
    }

    pub(crate) fn move_selection_to_horizontal_edge(&mut self, to_start: bool) -> bool {
        let Some(bounds) = self.movable_selection_bounds() else {
            return false;
        };
        let screen_width = self.screen_width.min(i32::MAX as u32) as i32;
        if screen_width <= 0 {
            return false;
        }

        let target_x = if to_start {
            0
        } else {
            screen_width - bounds.width
        };
        let dx = target_x - bounds.x;
        if dx == 0 {
            return false;
        }
        self.translate_selection_with_undo(dx, 0)
    }

    pub(crate) fn move_selection_to_vertical_edge(&mut self, to_start: bool) -> bool {
        let Some(bounds) = self.movable_selection_bounds() else {
            return false;
        };
        let screen_height = self.screen_height.min(i32::MAX as u32) as i32;
        if screen_height <= 0 {
            return false;
        }

        let target_y = if to_start {
            0
        } else {
            screen_height - bounds.height
        };
        let dy = target_y - bounds.y;
        if dy == 0 {
            return false;
        }
        self.translate_selection_with_undo(0, dy)
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

    pub(super) fn translate_shape(shape: &mut Shape, dx: i32, dy: i32) {
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
            Shape::StickyNote { x, y, .. } => {
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
}
