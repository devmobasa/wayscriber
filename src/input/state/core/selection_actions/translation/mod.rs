use crate::draw::ShapeId;
use crate::draw::frame::ShapeSnapshot;
use crate::input::InputState;

mod bounds;
mod transform;
mod undo;

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
}
