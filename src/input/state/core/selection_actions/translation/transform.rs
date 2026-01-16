use crate::draw::frame::ShapeSnapshot;
use crate::draw::{Shape, ShapeId};
use crate::input::InputState;

impl InputState {
    pub(crate) fn restore_selection_from_snapshots(
        &mut self,
        snapshots: Vec<(ShapeId, ShapeSnapshot)>,
    ) {
        if snapshots.is_empty() {
            return;
        }

        for (shape_id, snapshot) in snapshots {
            let bounds = {
                let frame = self.boards.active_frame_mut();
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

    pub(in crate::input::state::core::selection_actions) fn translate_shape(
        shape: &mut Shape,
        dx: i32,
        dy: i32,
    ) {
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
