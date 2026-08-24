mod grid;

use super::base::InputState;
use crate::draw::ShapeId;
use crate::input::hit_test;
use std::collections::{HashMap, HashSet};

pub(super) use self::grid::SpatialGrid;

impl InputState {
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
        if points.is_empty() {
            return Vec::new();
        }

        let frame = self.boards.active_frame();
        let len = frame.shapes.len();
        if len == 0 {
            return Vec::new();
        }

        // Build a lookup map for O(1) access by ShapeId (avoids O(n) per candidate)
        let shape_map: HashMap<ShapeId, &crate::draw::DrawnShape> =
            frame.shapes.iter().map(|s| (s.id, s)).collect();

        // Get candidate ShapeIds from spatial grid, or fall back to all shapes
        let candidate_ids: Vec<ShapeId> = if let Some(grid) = self.spatial_index.as_ref() {
            let mut unique = HashSet::new();
            for &(x, y) in points {
                // Scale query radius by tolerance to avoid false negatives
                for id in grid.query_with_tolerance((x, y), tolerance) {
                    unique.insert(id);
                }
            }
            unique.into_iter().collect()
        } else {
            // Fall back to all shapes
            frame.shapes.iter().map(|s| s.id).collect()
        };

        let mut hits = Vec::new();
        for id in candidate_ids {
            let Some(drawn) = shape_map.get(&id) else {
                continue;
            };
            let bounds = hit_test::compute_hit_bounds(drawn, tolerance);
            let hit = bounds
                .as_ref()
                .map(|rect| {
                    points.iter().any(|&(x, y)| {
                        rect.contains(x, y) && hit_test::hit_test(drawn, (x, y), tolerance)
                    })
                })
                .unwrap_or(false);
            if hit {
                hits.push(id);
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
            .and_then(|drawn| drawn.bounding_box());

        if let Some(grid) = &mut self.spatial_index {
            // Remove shape from its old cells
            grid.remove_shape(id);

            // Add shape to its new cells if it still exists
            if let Some(bounds) = new_bounds {
                grid.add_shape_with_bounds(id, bounds);
            }
        }
        // If no grid exists, it will be rebuilt on next query if needed
    }

    /// Updates the hit-test tolerance (in pixels).
    pub fn set_hit_test_tolerance(&mut self, tolerance: f64) {
        self.hit_test_tolerance = tolerance.max(1.0);
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

        // Only rebuild if no grid exists or shape count is way off (> 20% drift)
        let needs_rebuild = match &self.spatial_index {
            None => true,
            Some(grid) => {
                let drift = (grid.shape_count() as i64 - len as i64).unsigned_abs() as usize;
                drift > len / 5 + 1
            }
        };

        if needs_rebuild {
            let frame = self.boards.active_frame();
            self.spatial_index = SpatialGrid::build(frame);
        }
    }

    fn hit_test_single(&mut self, index: usize, x: i32, y: i32, tolerance: f64) -> Option<ShapeId> {
        let frame = self.boards.active_frame();
        if index >= frame.shapes.len() {
            return None;
        }

        let (shape_id, bounds, hit) = {
            let drawn = &frame.shapes[index];
            let cached = self.hit_test_cache.get(&drawn.id).copied();
            let bounds = cached.or_else(|| hit_test::compute_hit_bounds(drawn, tolerance));
            let hit = bounds
                .as_ref()
                .map(|rect| {
                    rect.contains(x, y)
                        && hit_test::hit_test_for_point_targeting(drawn, (x, y), tolerance)
                })
                .unwrap_or(false);
            (drawn.id, bounds, hit)
        };

        if let Some(bounds) = bounds {
            self.hit_test_cache.entry(shape_id).or_insert(bounds);
            if hit {
                return Some(shape_id);
            }
        }
        None
    }

    fn hit_test_by_id(&mut self, id: ShapeId, x: i32, y: i32, tolerance: f64) -> bool {
        let frame = self.boards.active_frame();
        let Some(drawn) = frame.shape(id) else {
            return false;
        };

        let cached = self.hit_test_cache.get(&id).copied();
        let bounds = cached.or_else(|| hit_test::compute_hit_bounds(drawn, tolerance));

        let hit = bounds
            .as_ref()
            .map(|rect| {
                rect.contains(x, y)
                    && hit_test::hit_test_for_point_targeting(drawn, (x, y), tolerance)
            })
            .unwrap_or(false);

        if let Some(bounds) = bounds {
            self.hit_test_cache.entry(id).or_insert(bounds);
        }

        hit
    }

    fn hit_test_indices<I>(&mut self, indices: I, x: i32, y: i32, tolerance: f64) -> Option<ShapeId>
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
        let tolerance = self.hit_test_tolerance;
        let len = self.boards.active_frame().shapes.len();
        let threshold = self.max_linear_hit_test;

        if len > threshold {
            self.ensure_spatial_index_for_active_frame();

            if let Some(grid) = &self.spatial_index {
                // Use tolerance-aware query to avoid false negatives
                let candidates = grid.query_with_tolerance((x, y), tolerance);

                // Build index map for O(1) lookup instead of O(n) find_index per candidate
                let frame = self.boards.active_frame();
                let index_map: HashMap<ShapeId, usize> = frame
                    .shapes
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (s.id, i))
                    .collect();

                // Sort candidates by their position in the frame (reverse for top-to-bottom)
                let mut sorted_candidates: Vec<_> = candidates
                    .into_iter()
                    .filter_map(|id| index_map.get(&id).map(|&idx| (idx, id)))
                    .collect();
                sorted_candidates.sort_unstable_by_key(|candidate| std::cmp::Reverse(candidate.0));

                for (_, id) in sorted_candidates {
                    if self.hit_test_by_id(id, x, y, tolerance) {
                        return Some(id);
                    }
                }
            }
        } else {
            self.spatial_index = None;
        }

        self.hit_test_indices((0..len).rev(), x, y, tolerance)
    }
}
