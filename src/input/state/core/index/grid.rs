use crate::draw::{Frame, ShapeId};
use crate::util::Rect;
use std::collections::{HashMap, HashSet};

const SPATIAL_GRID_CELL_SIZE: i32 = 64;
const MAX_SPATIAL_CELLS_PER_SHAPE: usize = 4_096;
// The largest centered odd square within this budget is 63 x 63 cells.
const MAX_SPATIAL_QUERY_CELLS: u64 = 4_096;
// Allows roughly 26 indexed cells per shape at the default 10,000-shape limit
// while placing a hard ceiling on duplicated `(shape, cell)` entries.
const MAX_SPATIAL_GRID_MEMBERSHIPS: usize = 262_144;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CellRange {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
    count: usize,
}

impl CellRange {
    fn keys(self) -> impl Iterator<Item = (i32, i32)> {
        (self.min_x..=self.max_x).flat_map(move |x| (self.min_y..=self.max_y).map(move |y| (x, y)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellCoverage {
    Bounded(CellRange),
    Global,
}

/// Spatial grid for efficient hit-testing using ShapeId instead of indices.
///
/// This allows incremental updates when shapes are added, removed, or modified
/// without needing to rebuild the entire grid.
#[derive(Debug, Clone)]
pub(in crate::input::state::core) struct SpatialGrid {
    cell_size: i32,
    /// Maps cell coordinates to the ShapeIds contained in that cell.
    cells: HashMap<(i32, i32), Vec<ShapeId>>,
    /// Reverse mapping from ShapeId to the cells it occupies for efficient removal.
    shape_cells: HashMap<ShapeId, Vec<(i32, i32)>>,
    /// Shapes kept as global candidates because their coverage is unsafe or
    /// oversized, or because the aggregate membership budget is exhausted.
    ///
    /// These remain candidates for every query, bounding per-shape index work
    /// and total grid memory without introducing hit-test false negatives.
    global_shapes: HashSet<ShapeId>,
    /// Number of `(shape, cell)` memberships currently stored in both maps.
    indexed_memberships: usize,
    /// Maximum number of memberships retained by this grid.
    max_indexed_memberships: usize,
    /// Number of shapes when the grid was built (for validation).
    shape_count: usize,
}

impl SpatialGrid {
    pub(super) fn build(frame: &Frame) -> Option<Self> {
        Self::build_with_membership_limit(
            frame,
            SPATIAL_GRID_CELL_SIZE,
            MAX_SPATIAL_GRID_MEMBERSHIPS,
        )
    }

    fn build_with_membership_limit(
        frame: &Frame,
        cell_size: i32,
        max_indexed_memberships: usize,
    ) -> Option<Self> {
        let cell_size = cell_size.max(1);
        if frame.shapes.is_empty() {
            return None;
        }

        let mut grid = Self {
            cell_size,
            cells: HashMap::new(),
            shape_cells: HashMap::new(),
            global_shapes: HashSet::new(),
            indexed_memberships: 0,
            max_indexed_memberships,
            shape_count: frame.shapes.len(),
        };

        for drawn in &frame.shapes {
            let Some(bounds) = drawn.bounding_box() else {
                continue;
            };
            grid.add_shape_with_bounds(drawn.id, bounds);
        }

        if grid.cells.is_empty() && grid.global_shapes.is_empty() {
            return None;
        }

        Some(grid)
    }

    pub(super) fn shape_count(&self) -> usize {
        self.shape_count
    }

    /// Computes bounded cell coverage without materializing its keys.
    fn compute_cell_coverage(bounds: Rect, cell_size: i32) -> CellCoverage {
        if !bounds.is_valid() {
            return CellCoverage::Global;
        }

        let cell_size = i64::from(cell_size.max(1));
        let min_cell_x = i64::from(bounds.x).div_euclid(cell_size);
        let max_cell_x = (i64::from(bounds.x) + i64::from(bounds.width) - 1).div_euclid(cell_size);
        let min_cell_y = i64::from(bounds.y).div_euclid(cell_size);
        let max_cell_y = (i64::from(bounds.y) + i64::from(bounds.height) - 1).div_euclid(cell_size);

        let columns = (max_cell_x - min_cell_x + 1) as u64;
        let rows = (max_cell_y - min_cell_y + 1) as u64;
        let Some(cell_count) = columns.checked_mul(rows) else {
            return CellCoverage::Global;
        };
        if cell_count > MAX_SPATIAL_CELLS_PER_SHAPE as u64 {
            return CellCoverage::Global;
        }

        let (Ok(min_cell_x), Ok(max_cell_x), Ok(min_cell_y), Ok(max_cell_y)) = (
            i32::try_from(min_cell_x),
            i32::try_from(max_cell_x),
            i32::try_from(min_cell_y),
            i32::try_from(max_cell_y),
        ) else {
            return CellCoverage::Global;
        };

        CellCoverage::Bounded(CellRange {
            min_x: min_cell_x,
            max_x: max_cell_x,
            min_y: min_cell_y,
            max_y: max_cell_y,
            count: cell_count as usize,
        })
    }

    /// Removes a shape from all cells it occupies.
    pub(super) fn remove_shape(&mut self, id: ShapeId) {
        self.global_shapes.remove(&id);
        if let Some(cell_keys) = self.shape_cells.remove(&id) {
            self.indexed_memberships = self
                .indexed_memberships
                .checked_sub(cell_keys.len())
                .expect("spatial index membership accounting underflow");
            for key in cell_keys {
                if let Some(ids) = self.cells.get_mut(&key) {
                    ids.retain(|&existing_id| existing_id != id);
                    if ids.is_empty() {
                        self.cells.remove(&key);
                    }
                }
            }
        }
    }

    /// Adds a shape with known bounds to the grid.
    pub(super) fn add_shape_with_bounds(&mut self, id: ShapeId, bounds: Rect) {
        match Self::compute_cell_coverage(bounds, self.cell_size) {
            CellCoverage::Bounded(cell_range)
                if cell_range.count
                    <= self
                        .max_indexed_memberships
                        .saturating_sub(self.indexed_memberships) =>
            {
                self.indexed_memberships += cell_range.count;
                let mut cell_keys = Vec::with_capacity(cell_range.count);
                for key in cell_range.keys() {
                    self.cells.entry(key).or_default().push(id);
                    cell_keys.push(key);
                }
                debug_assert_eq!(cell_keys.len(), cell_range.count);
                self.shape_cells.insert(id, cell_keys);
            }
            CellCoverage::Bounded(_) | CellCoverage::Global => {
                self.global_shapes.insert(id);
            }
        }
    }

    /// Queries for all ShapeIds in cells near the given point with tolerance-aware radius.
    ///
    /// The search radius is expanded based on tolerance to ensure shapes that could
    /// be hit within the tolerance distance are not missed.
    pub(super) fn query_with_tolerance(&self, point: (i32, i32), tolerance: f64) -> Vec<ShapeId> {
        let Some(radius) = Self::query_radius_within_budget(tolerance, self.cell_size) else {
            return self.all_candidates();
        };
        let cell_x = i64::from(point.0.div_euclid(self.cell_size));
        let cell_y = i64::from(point.1.div_euclid(self.cell_size));

        let mut unique = HashSet::with_capacity(self.global_shapes.len());
        unique.extend(self.global_shapes.iter().copied());
        for dx in -radius..=radius {
            let Ok(key_x) = i32::try_from(cell_x + dx) else {
                continue;
            };
            for dy in -radius..=radius {
                let Ok(key_y) = i32::try_from(cell_y + dy) else {
                    continue;
                };
                if let Some(ids) = self.cells.get(&(key_x, key_y)) {
                    unique.extend(ids.iter().copied());
                }
            }
        }

        unique.into_iter().collect()
    }

    fn query_radius_within_budget(tolerance: f64, cell_size: i32) -> Option<i64> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return None;
        }

        let extra_cells = (tolerance / f64::from(cell_size.max(1))).ceil();
        if extra_cells > (i64::MAX - 1) as f64 {
            return None;
        }
        let radius = (extra_cells as i64).checked_add(1)?;
        let diameter = radius.checked_mul(2)?.checked_add(1)?;
        let diameter = u64::try_from(diameter).ok()?;
        let query_cells = diameter.checked_mul(diameter)?;
        (query_cells <= MAX_SPATIAL_QUERY_CELLS).then_some(radius)
    }

    fn all_candidates(&self) -> Vec<ShapeId> {
        let mut candidates = Vec::with_capacity(self.shape_cells.len() + self.global_shapes.len());
        candidates.extend(self.shape_cells.keys().copied());
        candidates.extend(self.global_shapes.iter().copied());
        candidates
    }
}

#[cfg(test)]
mod tests;
