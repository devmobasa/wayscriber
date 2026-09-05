//! Selection resize functionality.

use crate::draw::ShapeId;
use crate::draw::TextMeasurer;
use crate::draw::frame::ShapeSnapshot;
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
        let measurer = TextMeasurer::default();
        self.hit_selection_handle_with(&measurer, x, y)
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
        self.capture_movable_selection_snapshots()
    }

    /// Apply resize transformation to all selected shapes.
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

        let edit = crate::input::state::core::editing::CanvasEdit::borrow_snapshots(snapshots);
        let effects = edit.preview(
            self.boards.active_frame_mut(),
            measurer,
            |shape, snapshot| {
                *shape = snapshot.shape.scaled(scale_x, scale_y, anchor_x, anchor_y);
                true
            },
        );
        self.apply_edit_effects(measurer, effects);
        self.mark_selection_dirty_region(self.selection_bounds_with(measurer));
    }

    /// Restore shapes from snapshots (used for cancel).
    pub(crate) fn restore_resize_from_snapshots_with(
        &mut self,
        measurer: &TextMeasurer,
        snapshots: &[(ShapeId, ShapeSnapshot)],
    ) {
        let previous_bounds = self.selection_bounds_with(measurer);
        self.restore_selection_from_snapshots_with(measurer, snapshots.to_vec());
        self.mark_selection_dirty_region(previous_bounds);
        self.mark_selection_dirty_region(self.selection_bounds_with(measurer));
    }
}
