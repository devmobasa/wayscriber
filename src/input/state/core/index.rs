//! Active-frame coordination for canvas hit testing and spatial indexing.

mod grid;
mod owner;

use super::base::InputState;
use crate::draw::{ShapeId, TextMeasurer, with_scoped_measurer};
use owner::ActiveFrameOrderGuard;
pub(in crate::input::state) use owner::CanvasIndex;
#[cfg(test)]
use owner::SPATIAL_SHAPE_INDEX_BUILDS;

impl InputState {
    #[cfg(test)]
    pub(crate) fn reset_spatial_shape_index_build_count() {
        SPATIAL_SHAPE_INDEX_BUILDS.with(|count| count.set(0));
    }

    #[cfg(test)]
    pub(crate) fn spatial_shape_index_build_count() -> usize {
        SPATIAL_SHAPE_INDEX_BUILDS.with(std::cell::Cell::get)
    }

    /// Returns all shapes intersecting any of the provided points within tolerance.
    pub(crate) fn hit_test_all_for_points_with(
        &mut self,
        measurer: &TextMeasurer,
        points: &[(i32, i32)],
        tolerance: f64,
    ) -> Vec<ShapeId> {
        self.ensure_spatial_index_for_active_frame_with(measurer);
        self.hit_test_all_for_points_cached_with(measurer, points, tolerance)
    }

    pub(crate) fn hit_test_all_for_points_cached_with(
        &self,
        measurer: &TextMeasurer,
        points: &[(i32, i32)],
        tolerance: f64,
    ) -> Vec<ShapeId> {
        let guard = self.active_frame_order_guard();
        self.canvas_index.hit_test_all_for_points(
            measurer,
            self.boards.active_frame(),
            guard,
            points,
            tolerance,
        )
    }

    /// Monotonic counter bumped whenever committed shape content may have changed.
    pub fn canvas_content_generation(&self) -> u64 {
        self.canvas_index.generation()
    }

    /// Clears all cached hit-test data and spatial index.
    pub fn invalidate_hit_cache(&mut self) {
        self.canvas_index.invalidate();
    }

    /// Incrementally updates cached hit-test data for a single shape.
    ///
    /// Instead of invalidating the entire spatial index, this method updates
    /// only the affected cells, providing O(1) amortized updates instead of O(n).
    pub fn invalidate_hit_cache_for(&mut self, id: ShapeId) {
        with_scoped_measurer(|measurer| self.invalidate_hit_cache_for_with(measurer, id))
    }

    /// Refreshes one shape in the index using the supplied text measurements.
    pub fn invalidate_hit_cache_for_with(&mut self, measurer: &TextMeasurer, id: ShapeId) {
        let new_bounds = self
            .boards
            .active_frame()
            .shape(id)
            .map(|drawn| drawn.bounding_box_with(measurer));
        let guard = self.active_frame_order_guard();
        self.canvas_index.invalidate_shape(id, new_bounds, guard);
    }

    /// Returns the configured hit-test tolerance in pixels.
    pub fn hit_test_tolerance(&self) -> f64 {
        self.canvas_index.tolerance()
    }

    /// Updates the hit-test tolerance (in pixels).
    pub fn set_hit_test_tolerance(&mut self, tolerance: f64) {
        self.canvas_index.set_tolerance(tolerance);
    }

    /// Updates the threshold used before building a spatial index.
    pub fn set_hit_test_threshold(&mut self, threshold: usize) {
        self.canvas_index.set_linear_threshold(threshold);
    }

    /// Maximum number of shapes accepted in one frame.
    pub fn max_shapes_per_frame(&self) -> usize {
        self.canvas_index.max_shapes_per_frame()
    }

    #[cfg(test)]
    pub(crate) fn set_max_shapes_per_frame_for_test(&mut self, limit: usize) {
        self.canvas_index.set_max_shapes_per_frame(limit);
    }

    /// Returns true if a spatial index is currently built and active.
    #[cfg(test)]
    pub fn has_spatial_index(&self) -> bool {
        self.canvas_index.has_spatial_index()
    }

    pub(crate) fn ensure_spatial_index_for_active_frame_with(&mut self, measurer: &TextMeasurer) {
        let guard = self.active_frame_order_guard();
        self.canvas_index
            .ensure_for_frame(measurer, self.boards.active_frame(), guard);
    }

    fn active_frame_order_guard(&self) -> ActiveFrameOrderGuard {
        ActiveFrameOrderGuard {
            board_identity_generation: self.boards.board_identity_generation(),
            board_index: self.boards.active_index(),
            page_generation: self.boards.active_page_generation(),
            page_index: self.boards.active_page_index(),
            shape_count: self.boards.active_frame().shapes.len(),
            shape_order_generation: self.boards.active_frame().shape_order_generation(),
        }
    }

    /// Performs hit-testing against the active frame and returns the top-most shape id.
    pub fn hit_test_at(&mut self, x: i32, y: i32) -> Option<ShapeId> {
        with_scoped_measurer(|measurer| self.hit_test_at_with(measurer, x, y))
    }

    /// Finds the topmost shape using the supplied canonical text measurements.
    pub fn hit_test_at_with(&mut self, measurer: &TextMeasurer, x: i32, y: i32) -> Option<ShapeId> {
        let guard = self.active_frame_order_guard();
        self.canvas_index
            .hit_test_at(measurer, self.boards.active_frame(), guard, x, y)
    }
}
#[cfg(test)]
mod measurement_tests;
