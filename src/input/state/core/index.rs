use super::base::InputState;
use crate::draw::{Frame, ShapeId};
use crate::input::hit_test;
use crate::util::Rect;
use std::collections::{HashMap, HashSet};

pub(super) const SPATIAL_GRID_CELL_SIZE: i32 = 64;

/// Spatial grid for efficient hit-testing using ShapeId instead of indices.
///
/// This allows incremental updates when shapes are added, removed, or modified
/// without needing to rebuild the entire grid.
#[derive(Debug, Clone)]
pub(super) struct SpatialGrid {
    pub(super) cell_size: i32,
    /// Maps cell coordinates to the ShapeIds contained in that cell.
    pub(super) cells: HashMap<(i32, i32), Vec<ShapeId>>,
    /// Reverse mapping from ShapeId to the cells it occupies for efficient removal.
    pub(super) shape_cells: HashMap<ShapeId, Vec<(i32, i32)>>,
    /// Number of shapes when the grid was built (for validation).
    pub(super) shape_count: usize,
}

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

        let frame = self.canvas_set.active_frame();
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

    /// Clears all cached hit-test data and spatial index.
    pub fn invalidate_hit_cache(&mut self) {
        self.hit_test_cache.clear();
        self.spatial_index = None;
    }

    /// Incrementally updates cached hit-test data for a single shape.
    ///
    /// Instead of invalidating the entire spatial index, this method updates
    /// only the affected cells, providing O(1) amortized updates instead of O(n).
    pub fn invalidate_hit_cache_for(&mut self, id: ShapeId) {
        self.hit_test_cache.remove(&id);

        // Get the shape's new bounds (if it still exists)
        let new_bounds = self
            .canvas_set
            .active_frame()
            .shape(id)
            .and_then(|drawn| drawn.shape.bounding_box());

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
        let len = self.canvas_set.active_frame().shapes.len();
        if len <= self.max_linear_hit_test {
            self.spatial_index = None;
            return;
        }

        // Only rebuild if no grid exists or shape count is way off (> 20% drift)
        let needs_rebuild = match &self.spatial_index {
            None => true,
            Some(grid) => {
                let drift = (grid.shape_count as i64 - len as i64).unsigned_abs() as usize;
                drift > len / 5 + 1
            }
        };

        if needs_rebuild {
            let frame = self.canvas_set.active_frame();
            self.spatial_index = SpatialGrid::build(frame, SPATIAL_GRID_CELL_SIZE);
        }
    }

    fn hit_test_single(&mut self, index: usize, x: i32, y: i32, tolerance: f64) -> Option<ShapeId> {
        let frame = self.canvas_set.active_frame();
        if index >= frame.shapes.len() {
            return None;
        }

        let (shape_id, bounds, hit) = {
            let drawn = &frame.shapes[index];
            let cached = self.hit_test_cache.get(&drawn.id).copied();
            let bounds = cached.or_else(|| hit_test::compute_hit_bounds(drawn, tolerance));
            let hit = bounds
                .as_ref()
                .map(|rect| rect.contains(x, y) && hit_test::hit_test(drawn, (x, y), tolerance))
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
        let frame = self.canvas_set.active_frame();
        let Some(drawn) = frame.shape(id) else {
            return false;
        };

        let cached = self.hit_test_cache.get(&id).copied();
        let bounds = cached.or_else(|| hit_test::compute_hit_bounds(drawn, tolerance));

        let hit = bounds
            .as_ref()
            .map(|rect| rect.contains(x, y) && hit_test::hit_test(drawn, (x, y), tolerance))
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
        let len = self.canvas_set.active_frame().shapes.len();
        let threshold = self.max_linear_hit_test;

        if len > threshold {
            self.ensure_spatial_index_for_active_frame();

            if let Some(grid) = &self.spatial_index {
                // Use tolerance-aware query to avoid false negatives
                let candidates = grid.query_with_tolerance((x, y), tolerance);

                // Build index map for O(1) lookup instead of O(n) find_index per candidate
                let frame = self.canvas_set.active_frame();
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
                sorted_candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0));

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

impl SpatialGrid {
    fn build(frame: &Frame, cell_size: i32) -> Option<Self> {
        let cell_size = cell_size.max(1);
        if frame.shapes.is_empty() {
            return None;
        }

        let mut cells: HashMap<(i32, i32), Vec<ShapeId>> = HashMap::new();
        let mut shape_cells: HashMap<ShapeId, Vec<(i32, i32)>> = HashMap::new();

        for drawn in &frame.shapes {
            let Some(bounds) = drawn.shape.bounding_box() else {
                continue;
            };

            let cell_keys = Self::compute_cell_keys(bounds, cell_size);
            for &key in &cell_keys {
                cells.entry(key).or_default().push(drawn.id);
            }
            shape_cells.insert(drawn.id, cell_keys);
        }

        if cells.is_empty() {
            return None;
        }

        Some(Self {
            cell_size,
            cells,
            shape_cells,
            shape_count: frame.shapes.len(),
        })
    }

    /// Computes the cell keys that a bounding box occupies.
    fn compute_cell_keys(bounds: Rect, cell_size: i32) -> Vec<(i32, i32)> {
        let min_cell_x = bounds.x.div_euclid(cell_size);
        let max_cell_x = (bounds.x + bounds.width - 1).div_euclid(cell_size);
        let min_cell_y = bounds.y.div_euclid(cell_size);
        let max_cell_y = (bounds.y + bounds.height - 1).div_euclid(cell_size);

        let mut keys = Vec::with_capacity(
            ((max_cell_x - min_cell_x + 1) * (max_cell_y - min_cell_y + 1)) as usize,
        );
        for cx in min_cell_x..=max_cell_x {
            for cy in min_cell_y..=max_cell_y {
                keys.push((cx, cy));
            }
        }
        keys
    }

    /// Removes a shape from all cells it occupies.
    fn remove_shape(&mut self, id: ShapeId) {
        if let Some(cell_keys) = self.shape_cells.remove(&id) {
            for key in cell_keys {
                if let Some(ids) = self.cells.get_mut(&key) {
                    ids.retain(|&existing_id| existing_id != id);
                    // Clean up empty cells
                    if ids.is_empty() {
                        self.cells.remove(&key);
                    }
                }
            }
        }
    }

    /// Adds a shape with known bounds to the grid.
    fn add_shape_with_bounds(&mut self, id: ShapeId, bounds: Rect) {
        let cell_keys = Self::compute_cell_keys(bounds, self.cell_size);
        for &key in &cell_keys {
            self.cells.entry(key).or_default().push(id);
        }
        self.shape_cells.insert(id, cell_keys);
    }

    /// Queries for all ShapeIds in cells near the given point with tolerance-aware radius.
    ///
    /// The search radius is expanded based on tolerance to ensure shapes that could
    /// be hit within the tolerance distance are not missed.
    fn query_with_tolerance(&self, point: (i32, i32), tolerance: f64) -> Vec<ShapeId> {
        let cell_x = point.0.div_euclid(self.cell_size);
        let cell_y = point.1.div_euclid(self.cell_size);

        // Expand search radius based on tolerance: ceil(tolerance / cell_size) + 1
        // The +1 ensures we always check at least the 3x3 neighborhood
        let extra_cells = (tolerance / self.cell_size as f64).ceil() as i32;
        let radius = 1 + extra_cells;

        let mut unique = HashSet::new();
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let key = (cell_x + dx, cell_y + dy);
                if let Some(ids) = self.cells.get(&key) {
                    unique.extend(ids.iter().copied());
                }
            }
        }

        unique.into_iter().collect()
    }
}
