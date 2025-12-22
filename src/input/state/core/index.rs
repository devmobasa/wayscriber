use super::base::InputState;
use crate::draw::{Frame, ShapeId};
use crate::input::hit_test;
use std::collections::{HashMap, HashSet};

pub(super) const SPATIAL_GRID_CELL_SIZE: i32 = 64;

#[derive(Debug, Clone)]
pub(super) struct SpatialGrid {
    pub(super) cell_size: i32,
    pub(super) cells: HashMap<(i32, i32), Vec<usize>>,
    pub(super) shape_count: usize,
}

impl InputState {
    /// Returns all shapes intersecting any of the provided points within tolerance.
    pub(crate) fn hit_test_all_for_points(
        &self,
        points: &[(i32, i32)],
        tolerance: f64,
    ) -> Vec<ShapeId> {
        if points.is_empty() {
            return Vec::new();
        }

        let mut hits = Vec::new();
        let frame = self.canvas_set.active_frame();
        for drawn in frame.shapes.iter().rev() {
            let Some(bounds) = hit_test::compute_hit_bounds(drawn, tolerance) else {
                continue;
            };
            let mut hit = false;
            for &(x, y) in points {
                if bounds.contains(x, y) && hit_test::hit_test(drawn, (x, y), tolerance) {
                    hit = true;
                    break;
                }
            }
            if hit {
                hits.push(drawn.id);
            }
        }
        hits
    }

    /// Clears cached hit-test bounds.
    pub fn invalidate_hit_cache(&mut self) {
        self.hit_test_cache.clear();
        self.spatial_index = None;
    }

    /// Removes cached hit-test data for a single shape.
    pub fn invalidate_hit_cache_for(&mut self, id: ShapeId) {
        self.hit_test_cache.remove(&id);
        self.spatial_index = None;
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
            let rebuild = !matches!(&self.spatial_index, Some(grid) if grid.shape_count == len);

            if rebuild {
                let frame = self.canvas_set.active_frame();
                self.spatial_index = SpatialGrid::build(frame, SPATIAL_GRID_CELL_SIZE);
            }

            if let Some(grid) = &self.spatial_index {
                let candidates = grid.query((x, y));
                if let Some(hit) = self.hit_test_indices(candidates, x, y, tolerance) {
                    return Some(hit);
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

        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();

        for (index, drawn) in frame.shapes.iter().enumerate() {
            let Some(bounds) = drawn.shape.bounding_box() else {
                continue;
            };

            let min_cell_x = bounds.x.div_euclid(cell_size);
            let max_cell_x = (bounds.x + bounds.width - 1).div_euclid(cell_size);
            let min_cell_y = bounds.y.div_euclid(cell_size);
            let max_cell_y = (bounds.y + bounds.height - 1).div_euclid(cell_size);

            for cx in min_cell_x..=max_cell_x {
                for cy in min_cell_y..=max_cell_y {
                    cells.entry((cx, cy)).or_default().push(index);
                }
            }
        }

        if cells.is_empty() {
            return None;
        }

        Some(Self {
            cell_size,
            cells,
            shape_count: frame.shapes.len(),
        })
    }

    fn query(&self, point: (i32, i32)) -> Vec<usize> {
        let cell_x = point.0.div_euclid(self.cell_size);
        let cell_y = point.1.div_euclid(self.cell_size);

        let mut unique = HashSet::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let key = (cell_x + dx, cell_y + dy);
                if let Some(indices) = self.cells.get(&key) {
                    unique.extend(indices.iter().copied());
                }
            }
        }

        let mut result: Vec<usize> = unique.into_iter().collect();
        result.sort_unstable_by(|a, b| b.cmp(a));
        result
    }
}
