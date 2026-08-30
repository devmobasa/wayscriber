mod grid;

use super::base::InputState;
use crate::draw::{Frame, ShapeId};
use crate::input::boards::BoardIdentityGeneration;
use crate::input::hit_test::{self, HitTestTolerance};
use std::collections::{HashMap, HashSet};

pub(super) use self::grid::SpatialGrid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveFrameOrderGuard {
    board_identity_generation: BoardIdentityGeneration,
    board_index: usize,
    page_generation: u64,
    page_index: usize,
    shape_count: usize,
    shape_order_generation: u64,
}

impl ActiveFrameOrderGuard {
    fn same_frame(self, other: Self) -> bool {
        self.board_identity_generation == other.board_identity_generation
            && self.board_index == other.board_index
            && self.page_generation == other.page_generation
            && self.page_index == other.page_index
    }
}

#[derive(Debug, Clone)]
pub(super) struct SpatialIndexCache {
    grid: SpatialGrid,
    shape_indices: Option<HashMap<ShapeId, usize>>,
    guard: ActiveFrameOrderGuard,
}

#[cfg(test)]
std::thread_local! {
    static SPATIAL_SHAPE_INDEX_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

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
    pub(crate) fn hit_test_all_for_points(
        &mut self,
        points: &[(i32, i32)],
        tolerance: f64,
    ) -> Vec<ShapeId> {
        self.ensure_spatial_index_for_active_frame();
        self.hit_test_all_for_points_cached(points, tolerance)
    }

    /// Returns all shapes intersecting any of the provided points using cached spatial data.
    pub(crate) fn hit_test_all_for_points_cached(
        &self,
        points: &[(i32, i32)],
        tolerance: f64,
    ) -> Vec<ShapeId> {
        let Some(tolerance) = HitTestTolerance::new(tolerance) else {
            return Vec::new();
        };

        if points.is_empty() {
            return Vec::new();
        }

        let frame = self.boards.active_frame();
        let len = frame.shapes.len();
        if len == 0 {
            return Vec::new();
        }

        // Resolve grid candidates through the cached z-order map. If no
        // complete index is available, walking the frame directly avoids
        // building a throwaway ShapeId map for this query.
        let guard = self.active_frame_order_guard();
        let candidate_indices: Vec<usize> = if let Some((index, shape_indices)) = self
            .spatial_index
            .as_ref()
            .filter(|index| index.guard == guard)
            .and_then(|index| index.shape_indices.as_ref().map(|indices| (index, indices)))
        {
            let mut unique = HashSet::new();
            for &(x, y) in points {
                // Scale query radius by tolerance to avoid false negatives
                for id in index.grid.query_with_tolerance((x, y), tolerance) {
                    unique.insert(id);
                }
            }
            let mut indices = Vec::with_capacity(unique.len());
            let stale = unique.into_iter().any(|id| {
                let Some(shape_index) = shape_indices.get(&id).copied() else {
                    return true;
                };
                if frame
                    .shapes
                    .get(shape_index)
                    .is_none_or(|shape| shape.id != id)
                {
                    return true;
                }
                indices.push(shape_index);
                false
            });
            if stale { (0..len).collect() } else { indices }
        } else {
            // Fall back to all shapes
            (0..len).collect()
        };

        let mut hits = Vec::new();
        for index in candidate_indices {
            let Some(drawn) = frame.shapes.get(index) else {
                continue;
            };
            let bounds = hit_test::compute_hit_bounds_with_tolerance(drawn, tolerance);
            let hit = points.iter().any(|&(x, y)| {
                bounds.as_ref().is_none_or(|rect| rect.contains(x, y))
                    && hit_test::hit_test_with_tolerance(drawn, (x, y), tolerance)
            });
            if hit {
                hits.push(drawn.id);
            }
        }

        hits
    }

    /// Monotonic counter bumped whenever committed shape content may have changed.
    pub fn canvas_content_generation(&self) -> u64 {
        self.canvas_content_generation
    }

    /// Clears all cached hit-test data and spatial index.
    pub fn invalidate_hit_cache(&mut self) {
        self.canvas_content_generation = self.canvas_content_generation.wrapping_add(1);
        self.hit_test_cache.clear();
        self.spatial_index = None;
    }

    /// Incrementally updates cached hit-test data for a single shape.
    ///
    /// Instead of invalidating the entire spatial index, this method updates
    /// only the affected cells, providing O(1) amortized updates instead of O(n).
    pub fn invalidate_hit_cache_for(&mut self, id: ShapeId) {
        self.canvas_content_generation = self.canvas_content_generation.wrapping_add(1);
        self.hit_test_cache.remove(&id);

        // Get the shape's new bounds (if it still exists)
        let new_bounds = self
            .boards
            .active_frame()
            .shape(id)
            .map(|drawn| drawn.bounding_box());
        let guard = self.active_frame_order_guard();

        if self
            .spatial_index
            .as_ref()
            .is_some_and(|index| !index.guard.same_frame(guard))
        {
            self.spatial_index = None;
            return;
        }

        // `Frame::shapes` is public compatibility storage. If a caller
        // replaced or inserted an id directly, we cannot know which old grid
        // entry to remove, so rebuild the complete spatial cache.
        if self
            .spatial_index
            .as_ref()
            .and_then(|index| index.shape_indices.as_ref())
            .is_some_and(|shape_indices| !shape_indices.contains_key(&id))
        {
            self.spatial_index = None;
            return;
        }

        if let Some(index) = &mut self.spatial_index {
            // Remove shape from its old cells
            index.grid.remove_shape(id);

            // Add shape to its new cells, or retain it as a global candidate
            // when its full bounds cannot be represented by Rect.
            if let Some(bounds) = new_bounds {
                index.grid.add_shape(id, bounds);
            }

            if index.guard.shape_count != guard.shape_count
                || index.guard.shape_order_generation != guard.shape_order_generation
            {
                index.shape_indices = None;
            }
            index.guard = guard;
        }
        // If no grid exists, it will be rebuilt on next query if needed
    }

    /// Updates the hit-test tolerance (in pixels).
    pub fn set_hit_test_tolerance(&mut self, tolerance: f64) {
        self.hit_test_tolerance = HitTestTolerance::new(tolerance)
            .unwrap_or(HitTestTolerance::ONE_PIXEL)
            .at_least(HitTestTolerance::ONE_PIXEL)
            .value();
        self.invalidate_hit_cache();
    }

    /// Updates the threshold used before building a spatial index.
    pub fn set_hit_test_threshold(&mut self, threshold: usize) {
        self.max_linear_hit_test = threshold.max(1);
    }

    /// Returns true if a spatial index is currently built and active.
    #[cfg(test)]
    pub fn has_spatial_index(&self) -> bool {
        self.spatial_index.is_some()
    }

    pub(crate) fn ensure_spatial_index_for_active_frame(&mut self) {
        let len = self.boards.active_frame().shapes.len();
        if len <= self.max_linear_hit_test {
            self.spatial_index = None;
            return;
        }
        let guard = self.active_frame_order_guard();

        // Rebuild when the active frame or its z-order changed without passing
        // through incremental invalidation, or shape count drift is excessive.
        let needs_rebuild = match &self.spatial_index {
            None => true,
            Some(index) => {
                let drift = (index.grid.shape_count() as i64 - len as i64).unsigned_abs() as usize;
                index.guard != guard || drift > len / 5 + 1
            }
        };

        if needs_rebuild {
            let frame = self.boards.active_frame();
            self.spatial_index = SpatialGrid::build(frame).map(|grid| SpatialIndexCache {
                grid,
                shape_indices: None,
                guard,
            });
        }

        if self
            .spatial_index
            .as_ref()
            .is_some_and(|index| index.shape_indices.is_none())
        {
            let shape_indices = Self::build_spatial_shape_indices(self.boards.active_frame());
            if let Some(index) = &mut self.spatial_index {
                index.shape_indices = Some(shape_indices);
            }
        }
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

    fn build_spatial_shape_indices(frame: &Frame) -> HashMap<ShapeId, usize> {
        #[cfg(test)]
        SPATIAL_SHAPE_INDEX_BUILDS.with(|count| count.set(count.get().saturating_add(1)));
        frame
            .shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| (shape.id, index))
            .collect()
    }

    fn hit_test_single(
        &mut self,
        index: usize,
        x: i32,
        y: i32,
        tolerance: HitTestTolerance,
    ) -> Option<ShapeId> {
        let frame = self.boards.active_frame();
        if index >= frame.shapes.len() {
            return None;
        }

        let (shape_id, bounds, hit) = {
            let drawn = &frame.shapes[index];
            let cached = self.hit_test_cache.get(&drawn.id).copied();
            let bounds =
                cached.or_else(|| hit_test::compute_hit_bounds_with_tolerance(drawn, tolerance));
            let hit = bounds.as_ref().is_none_or(|rect| rect.contains(x, y))
                && hit_test::hit_test_for_point_targeting_with_tolerance(drawn, (x, y), tolerance);
            (drawn.id, bounds, hit)
        };

        if let Some(bounds) = bounds {
            self.hit_test_cache.entry(shape_id).or_insert(bounds);
        }
        if hit {
            return Some(shape_id);
        }
        None
    }

    fn hit_test_indices<I>(
        &mut self,
        indices: I,
        x: i32,
        y: i32,
        tolerance: HitTestTolerance,
    ) -> Option<ShapeId>
    where
        I: IntoIterator<Item = usize>,
    {
        for index in indices {
            if let Some(shape_id) = self.hit_test_single(index, x, y, tolerance) {
                return Some(shape_id);
            }
        }
        None
    }

    /// Performs hit-testing against the active frame and returns the top-most shape id.
    pub fn hit_test_at(&mut self, x: i32, y: i32) -> Option<ShapeId> {
        let tolerance =
            HitTestTolerance::new(self.hit_test_tolerance).unwrap_or(HitTestTolerance::ONE_PIXEL);
        let len = self.boards.active_frame().shapes.len();
        let threshold = self.max_linear_hit_test;

        if len > threshold {
            self.ensure_spatial_index_for_active_frame();

            if let Some(index) = &self.spatial_index {
                // Use tolerance-aware query to avoid false negatives
                let candidates = index.grid.query_with_tolerance((x, y), tolerance);
                let index_map = index
                    .shape_indices
                    .as_ref()
                    .expect("spatial shape indices are built with the grid");

                // Sort candidates by their position in the frame (reverse for top-to-bottom).
                // Public `Frame::shapes` access remains compatibility API, so
                // fail over to the linear path if a caller bypassed Frame's
                // generation-tracked mutation methods.
                let frame = self.boards.active_frame();
                let mut stale = false;
                let mut sorted_candidates: Vec<_> = candidates
                    .into_iter()
                    .filter_map(|id| {
                        let Some(shape_index) = index_map.get(&id).copied() else {
                            stale = true;
                            return None;
                        };
                        if frame
                            .shapes
                            .get(shape_index)
                            .is_none_or(|shape| shape.id != id)
                        {
                            stale = true;
                            return None;
                        }
                        Some(shape_index)
                    })
                    .collect();
                sorted_candidates.sort_unstable_by_key(|&index| std::cmp::Reverse(index));

                if !stale
                    && let Some(id) = self.hit_test_indices(sorted_candidates, x, y, tolerance)
                {
                    return Some(id);
                }
            }
        } else {
            self.spatial_index = None;
        }

        self.hit_test_indices((0..len).rev(), x, y, tolerance)
    }
}
