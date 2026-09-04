//! Selection resize functionality.

use crate::draw::ShapeId;
use crate::draw::frame::ShapeSnapshot;
use crate::draw::{TextMeasurer, with_legacy_measurer};
use crate::input::InputState;
use crate::input::state::core::base::SelectionHandle;
use crate::util::Rect;
mod resize_helpers;

// Handle size for hit testing (matches render constants)
const HANDLE_SIZE: i32 = 8;
const HANDLE_TOLERANCE: i32 = 4;

impl InputState {
    /// Hit test for selection handles. Returns the handle if mouse is over one.
    pub fn hit_selection_handle(&self, x: i32, y: i32) -> Option<SelectionHandle> {
        with_legacy_measurer(|measurer| self.hit_selection_handle_with(measurer, x, y))
    }

    /// Hit-tests selection handles using the supplied text measurement owner.
    pub fn hit_selection_handle_with(
        &self,
        measurer: &TextMeasurer,
        x: i32,
        y: i32,
    ) -> Option<SelectionHandle> {
        let bounds = self.selection_bounds_with(measurer)?;
        let corner_radius = (HANDLE_SIZE / 2) + HANDLE_TOLERANCE;
        let edge_radius = (HANDLE_SIZE * 3 / 4) / 2 + HANDLE_TOLERANCE;

        Self::selection_handle_probes(&bounds, corner_radius, edge_radius)
            .into_iter()
            .find_map(|probe| {
                self.point_near(x, y, probe.x, probe.y, probe.radius)
                    .then_some(probe.handle)
            })
    }

    /// Capture snapshots of selected shapes for resize operation.
    pub(crate) fn capture_resize_selection_snapshots(&self) -> Vec<(ShapeId, ShapeSnapshot)> {
        let ids = self.selected_shape_ids();
        let frame = self.boards.active_frame();
        let mut snapshots = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(shape) = frame.shape(*id)
                && !shape.locked
            {
                snapshots.push((
                    *id,
                    ShapeSnapshot {
                        shape: shape.shape.clone(),
                        locked: shape.locked,
                    },
                ));
            }
        }
        snapshots
    }

    /// Apply resize transformation to all selected shapes.
    pub(crate) fn apply_selection_resize(
        &mut self,
        handle: SelectionHandle,
        original_bounds: &Rect,
        dx: i32,
        dy: i32,
        snapshots: &[(ShapeId, ShapeSnapshot)],
    ) {
        with_legacy_measurer(|measurer| {
            self.apply_selection_resize_with(measurer, handle, original_bounds, dx, dy, snapshots)
        })
    }

    pub(crate) fn apply_selection_resize_with(
        &mut self,
        measurer: &TextMeasurer,
        handle: SelectionHandle,
        original_bounds: &Rect,
        dx: i32,
        dy: i32,
        snapshots: &[(ShapeId, ShapeSnapshot)],
    ) {
        if snapshots.is_empty() {
            return;
        }

        let previous_bounds = self.selection_bounds_with(measurer);
        self.mark_selection_dirty_region(previous_bounds);
        // Calculate scale factors based on handle and delta
        let (scale_x, scale_y, anchor_x, anchor_y) =
            Self::compute_scale_factors(handle, original_bounds, dx, dy);

        // Collect IDs to invalidate after the loop
        let mut ids_to_invalidate = Vec::with_capacity(snapshots.len());

        {
            let frame = self.boards.active_frame_mut();
            for (shape_id, snapshot) in snapshots {
                if let Some(drawn) = frame.shape_mut(*shape_id) {
                    // Apply scaling transformation to the shape
                    drawn.set_shape(snapshot.shape.scaled(scale_x, scale_y, anchor_x, anchor_y));
                    ids_to_invalidate.push(*shape_id);
                }
            }
        }

        for shape_id in ids_to_invalidate {
            self.invalidate_hit_cache_for_with(measurer, shape_id);
        }
        self.mark_selection_dirty_region(self.selection_bounds_with(measurer));
    }

    /// Restore shapes from snapshots (used for cancel).
    pub(crate) fn restore_resize_from_snapshots_with(
        &mut self,
        measurer: &TextMeasurer,
        snapshots: &[(ShapeId, ShapeSnapshot)],
    ) {
        let previous_bounds = self.selection_bounds_with(measurer);
        let mut ids_to_invalidate = Vec::with_capacity(snapshots.len());

        {
            let frame = self.boards.active_frame_mut();
            for (shape_id, snapshot) in snapshots {
                if let Some(drawn) = frame.shape_mut(*shape_id) {
                    drawn.set_shape(snapshot.shape.clone());
                    drawn.locked = snapshot.locked;
                    ids_to_invalidate.push(*shape_id);
                }
            }
        }

        self.mark_selection_dirty_region(previous_bounds);
        self.mark_selection_dirty_region(self.selection_bounds_with(measurer));
        for shape_id in ids_to_invalidate {
            self.invalidate_hit_cache_for_with(measurer, shape_id);
        }
        self.needs_redraw = true;
    }
}
