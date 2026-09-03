use crate::draw::ShapeId;
use crate::draw::frame::ShapeSnapshot;
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
                    let before = shape.bounding_box();
                    shape.set_shape(snapshot.shape);
                    shape.locked = snapshot.locked;
                    let after = shape.bounding_box();
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
}
