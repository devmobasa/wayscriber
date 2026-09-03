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

#[derive(Debug, Clone)]
pub(in crate::input::state) struct CanvasIndex {
    hit_test_cache: HashMap<ShapeId, crate::util::Rect>,
    content_generation: u64,
    tolerance: f64,
    linear_threshold: usize,
    spatial_index: Option<SpatialIndexCache>,
    max_shapes_per_frame: usize,
}

impl Default for CanvasIndex {
    fn default() -> Self {
        Self {
            hit_test_cache: HashMap::new(),
            content_generation: 0,
            tolerance: 6.0,
            linear_threshold: 400,
            spatial_index: None,
            max_shapes_per_frame: 0,
        }
    }
}

impl CanvasIndex {
    pub(in crate::input::state) fn from_config(
        hit_test_tolerance: f64,
        max_shapes_per_frame: usize,
    ) -> Self {
        let mut index = Self {
            max_shapes_per_frame,
            ..Self::default()
        };
        index.tolerance = Self::normalized_tolerance(hit_test_tolerance);
        index
    }

    pub(in crate::input::state) fn generation(&self) -> u64 {
        self.content_generation
    }

    pub(in crate::input::state) fn invalidate(&mut self) {
        self.content_generation = self.content_generation.wrapping_add(1);
        self.hit_test_cache.clear();
        self.spatial_index = None;
    }

    fn invalidate_shape(
        &mut self,
        id: ShapeId,
        new_bounds: Option<Option<crate::util::Rect>>,
        guard: ActiveFrameOrderGuard,
    ) {
        self.content_generation = self.content_generation.wrapping_add(1);
        self.hit_test_cache.remove(&id);

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
            index.grid.remove_shape(id);
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
    }

    fn ensure_for_frame(&mut self, frame: &Frame, guard: ActiveFrameOrderGuard) {
        let len = frame.shapes.len();
        if len <= self.linear_threshold {
            self.spatial_index = None;
            return;
        }

        let needs_rebuild = match &self.spatial_index {
            None => true,
            Some(index) => {
                let drift = (index.grid.shape_count() as i64 - len as i64).unsigned_abs() as usize;
                index.guard != guard || drift > len / 5 + 1
            }
        };

        if needs_rebuild {
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
            let shape_indices = Self::build_spatial_shape_indices(frame);
            if let Some(index) = &mut self.spatial_index {
                index.shape_indices = Some(shape_indices);
            }
        }
    }

    fn hit_test_all_for_points(
        &self,
        frame: &Frame,
        guard: ActiveFrameOrderGuard,
        points: &[(i32, i32)],
        tolerance: f64,
    ) -> Vec<ShapeId> {
        let Some(tolerance) = HitTestTolerance::new(tolerance) else {
            return Vec::new();
        };
        if points.is_empty() || frame.shapes.is_empty() {
            return Vec::new();
        }

        let len = frame.shapes.len();
        let candidate_indices: Vec<usize> = if let Some((index, shape_indices)) = self
            .spatial_index
            .as_ref()
            .filter(|index| index.guard == guard)
            .and_then(|index| index.shape_indices.as_ref().map(|indices| (index, indices)))
        {
            let mut unique = HashSet::new();
            for &(x, y) in points {
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

    fn hit_test_at(
        &mut self,
        frame: &Frame,
        guard: ActiveFrameOrderGuard,
        x: i32,
        y: i32,
    ) -> Option<ShapeId> {
        let tolerance =
            HitTestTolerance::new(self.tolerance).unwrap_or(HitTestTolerance::ONE_PIXEL);
        let len = frame.shapes.len();

        if len > self.linear_threshold {
            self.ensure_for_frame(frame, guard);
            if let Some(index) = &self.spatial_index {
                let candidates = index.grid.query_with_tolerance((x, y), tolerance);
                let index_map = index
                    .shape_indices
                    .as_ref()
                    .expect("spatial shape indices are built with the grid");
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
                    && let Some(id) =
                        self.hit_test_indices(frame, sorted_candidates, x, y, tolerance)
                {
                    return Some(id);
                }
            }
        } else {
            self.spatial_index = None;
        }

        self.hit_test_indices(frame, (0..len).rev(), x, y, tolerance)
    }

    pub(in crate::input::state) fn tolerance(&self) -> f64 {
        self.tolerance
    }

    pub(in crate::input::state) fn set_tolerance(&mut self, tolerance: f64) {
        self.tolerance = Self::normalized_tolerance(tolerance);
        self.invalidate();
    }

    pub(in crate::input::state) fn set_linear_threshold(&mut self, threshold: usize) {
        self.linear_threshold = threshold.max(1);
    }

    pub(in crate::input::state) fn max_shapes_per_frame(&self) -> usize {
        self.max_shapes_per_frame
    }

    #[cfg(test)]
    pub(in crate::input::state) fn set_max_shapes_per_frame(&mut self, limit: usize) {
        self.max_shapes_per_frame = limit;
    }

    #[cfg(test)]
    fn has_spatial_index(&self) -> bool {
        self.spatial_index.is_some()
    }

    fn normalized_tolerance(tolerance: f64) -> f64 {
        HitTestTolerance::new(tolerance)
            .unwrap_or(HitTestTolerance::ONE_PIXEL)
            .at_least(HitTestTolerance::ONE_PIXEL)
            .value()
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
        frame: &Frame,
        index: usize,
        x: i32,
        y: i32,
        tolerance: HitTestTolerance,
    ) -> Option<ShapeId> {
        let drawn = frame.shapes.get(index)?;
        let cached = self.hit_test_cache.get(&drawn.id).copied();
        let bounds =
            cached.or_else(|| hit_test::compute_hit_bounds_with_tolerance(drawn, tolerance));
        let hit = bounds.as_ref().is_none_or(|rect| rect.contains(x, y))
            && hit_test::hit_test_for_point_targeting_with_tolerance(drawn, (x, y), tolerance);
        if let Some(bounds) = bounds {
            self.hit_test_cache.entry(drawn.id).or_insert(bounds);
        }
        hit.then_some(drawn.id)
    }

    fn hit_test_indices<I>(
        &mut self,
        frame: &Frame,
        indices: I,
        x: i32,
        y: i32,
        tolerance: HitTestTolerance,
    ) -> Option<ShapeId>
    where
        I: IntoIterator<Item = usize>,
    {
        for index in indices {
            if let Some(shape_id) = self.hit_test_single(frame, index, x, y, tolerance) {
                return Some(shape_id);
            }
        }
        None
    }
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
        let guard = self.active_frame_order_guard();
        self.canvas_index.hit_test_all_for_points(
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
        let new_bounds = self
            .boards
            .active_frame()
            .shape(id)
            .map(|drawn| drawn.bounding_box());
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

    pub(crate) fn ensure_spatial_index_for_active_frame(&mut self) {
        let guard = self.active_frame_order_guard();
        self.canvas_index
            .ensure_for_frame(self.boards.active_frame(), guard);
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
        let guard = self.active_frame_order_guard();
        self.canvas_index
            .hit_test_at(self.boards.active_frame(), guard, x, y)
    }
}

#[cfg(test)]
mod canvas_index_owner_tests {
    use super::*;
    use crate::draw::{Color, Shape};
    use crate::util::Rect;

    fn frame_with_rectangles(count: usize) -> Frame {
        let mut frame = Frame::new();
        for offset in 0..count as i32 {
            frame.add_shape(Shape::Rect {
                x: offset * 20,
                y: 0,
                w: 10,
                h: 10,
                color: Color::new(0.0, 0.0, 0.0, 1.0),
                thick: 2.0,
                fill: false,
            });
        }
        frame
    }

    fn guard(identity: u64, frame: &Frame) -> ActiveFrameOrderGuard {
        ActiveFrameOrderGuard {
            board_identity_generation: BoardIdentityGeneration(identity),
            board_index: 0,
            page_generation: 1,
            page_index: 0,
            shape_count: frame.shapes.len(),
            shape_order_generation: frame.shape_order_generation(),
        }
    }

    #[test]
    fn invalidating_a_shape_with_a_stale_frame_guard_drops_the_index() {
        let frame = frame_with_rectangles(2);
        let mut index = CanvasIndex::default();
        index.set_linear_threshold(1);
        index.ensure_for_frame(&frame, guard(1, &frame));
        assert!(index.has_spatial_index());

        index.invalidate_shape(
            frame.shapes[0].id,
            Some(Some(Rect::new(0, 0, 10, 10).unwrap())),
            guard(2, &frame),
        );

        assert!(!index.has_spatial_index());
    }

    #[test]
    fn tolerance_and_linear_threshold_are_floored_at_one() {
        let mut index = CanvasIndex::from_config(f64::NAN, 20);
        assert_eq!(index.tolerance(), 1.0);
        index.set_tolerance(-4.0);
        assert_eq!(index.tolerance(), 1.0);

        index.set_linear_threshold(0);
        let frame = frame_with_rectangles(1);
        index.ensure_for_frame(&frame, guard(1, &frame));
        assert!(!index.has_spatial_index());
    }

    #[test]
    fn ensuring_a_frame_below_the_threshold_clears_an_existing_index() {
        let frame = frame_with_rectangles(2);
        let mut index = CanvasIndex::default();
        index.set_linear_threshold(1);
        index.ensure_for_frame(&frame, guard(1, &frame));
        assert!(index.has_spatial_index());

        index.set_linear_threshold(2);
        index.ensure_for_frame(&frame, guard(1, &frame));
        assert!(!index.has_spatial_index());
    }
}
