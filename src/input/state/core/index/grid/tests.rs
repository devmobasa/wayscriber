use super::*;
use crate::draw::{Color, Shape};
use crate::input::hit_test::HitTestTolerance;

fn tolerance(value: f64) -> HitTestTolerance {
    HitTestTolerance::new(value).expect("valid test tolerance")
}

fn filled_rect(x: i32, y: i32, width: i32, height: i32) -> Shape {
    Shape::Rect {
        x,
        y,
        w: width,
        h: height,
        fill: true,
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    }
}

fn assert_membership_accounting(grid: &SpatialGrid) {
    let reverse_memberships = grid.shape_cells.values().map(Vec::len).sum::<usize>();
    let forward_memberships = grid.cells.values().map(Vec::len).sum::<usize>();
    assert_eq!(grid.indexed_memberships, reverse_memberships);
    assert_eq!(grid.indexed_memberships, forward_memberships);
    assert!(grid.indexed_memberships <= grid.max_indexed_memberships);
    for global_id in &grid.global_shapes {
        assert!(!grid.shape_cells.contains_key(global_id));
        assert!(grid.cells.values().all(|ids| !ids.contains(global_id)));
    }
}

fn mixed_candidate_grid() -> (SpatialGrid, [ShapeId; 3]) {
    let mut frame = Frame::new();
    let first = frame.add_shape(filled_rect(10, 10, 10, 10));
    let second = frame.add_shape(filled_rect(74, 10, 10, 10));
    let third = frame.add_shape(filled_rect(138, 10, 10, 10));
    let grid = SpatialGrid::build_with_membership_limit(
        &TextMeasurer::default(),
        &frame,
        SPATIAL_GRID_CELL_SIZE,
        2,
    )
    .expect("spatial grid");
    (grid, [first, second, third])
}

fn candidate_set(candidates: Vec<ShapeId>) -> HashSet<ShapeId> {
    candidates.into_iter().collect()
}

#[test]
fn oversized_shape_is_queried_without_per_cell_index_entries() {
    let mut frame = Frame::new();
    let shape_id = frame.add_shape(filled_rect(0, 0, 100_000, 100_000));

    let grid = SpatialGrid::build(&TextMeasurer::default(), &frame).expect("spatial grid");

    assert!(grid.cells.is_empty());
    assert!(grid.shape_cells.is_empty());
    assert_eq!(grid.global_shapes, HashSet::from([shape_id]));
    assert!(
        grid.query_with_tolerance((50_000, 50_000), tolerance(1.0))
            .contains(&shape_id)
    );
}

#[test]
fn shape_without_bounds_remains_a_global_candidate() {
    let mut frame = Frame::new();
    let shape_id = frame.add_shape(Shape::Freehand {
        points: Vec::new(),
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        thick: 1.0,
    });
    assert!(
        frame
            .shape(shape_id)
            .and_then(|shape| shape.bounding_box())
            .is_none()
    );

    let grid = SpatialGrid::build(&TextMeasurer::default(), &frame).expect("spatial grid");

    assert!(grid.cells.is_empty());
    assert!(grid.shape_cells.is_empty());
    assert_eq!(grid.global_shapes, HashSet::from([shape_id]));
    assert!(
        grid.query_with_tolerance((i32::MIN, 10), tolerance(1.0))
            .contains(&shape_id)
    );
}

#[test]
fn cell_coverage_uses_wide_arithmetic_near_coordinate_limits() {
    let bounds = Rect::new(i32::MAX - 31, i32::MAX - 31, 64, 64).expect("valid bounds");

    assert_eq!(
        SpatialGrid::compute_cell_coverage(bounds, 1),
        CellCoverage::Global
    );
}

#[test]
fn invalid_bounds_remain_conservative_global_candidates() {
    let bounds = Rect {
        x: 10,
        y: 20,
        width: 0,
        height: 30,
    };

    assert_eq!(
        SpatialGrid::compute_cell_coverage(bounds, SPATIAL_GRID_CELL_SIZE),
        CellCoverage::Global
    );
}

#[test]
fn bounded_coverage_reports_count_before_key_materialization() {
    let bounds = Rect::new(0, 0, 128, 128).expect("valid bounds");
    let CellCoverage::Bounded(cell_range) =
        SpatialGrid::compute_cell_coverage(bounds, SPATIAL_GRID_CELL_SIZE)
    else {
        panic!("expected bounded cell coverage");
    };

    assert_eq!(cell_range.count, 4);
    assert_eq!(
        cell_range.keys().collect::<Vec<_>>(),
        vec![(0, 0), (0, 1), (1, 0), (1, 1)]
    );
}

#[test]
fn bounded_coverage_handles_negative_exact_cell_edges() {
    let bounds = Rect::new(-64, -64, 128, 64).expect("valid bounds");
    let CellCoverage::Bounded(cell_range) =
        SpatialGrid::compute_cell_coverage(bounds, SPATIAL_GRID_CELL_SIZE)
    else {
        panic!("expected bounded cell coverage");
    };

    assert_eq!(cell_range.count, 2);
    assert_eq!(
        cell_range.keys().collect::<Vec<_>>(),
        vec![(-1, -1), (0, -1)]
    );
}

#[test]
fn cell_coverage_enforces_per_shape_limit_boundary() {
    let at_limit = Rect::new(0, 0, MAX_SPATIAL_CELLS_PER_SHAPE as i32, 1).expect("valid bounds");
    let over_limit =
        Rect::new(0, 0, MAX_SPATIAL_CELLS_PER_SHAPE as i32 + 1, 1).expect("valid bounds");

    let CellCoverage::Bounded(cell_range) = SpatialGrid::compute_cell_coverage(at_limit, 1) else {
        panic!("coverage at the limit should remain bounded");
    };
    assert_eq!(cell_range.count, MAX_SPATIAL_CELLS_PER_SHAPE);
    assert_eq!(
        SpatialGrid::compute_cell_coverage(over_limit, 1),
        CellCoverage::Global
    );
}

#[test]
fn oversized_shape_can_move_back_into_regular_cells() {
    let mut frame = Frame::new();
    let shape_id = frame.add_shape(filled_rect(0, 0, 100_000, 100_000));
    let mut grid = SpatialGrid::build(&TextMeasurer::default(), &frame).expect("spatial grid");

    grid.remove_shape(shape_id);
    grid.add_shape_with_bounds(shape_id, Rect::new(128, 128, 32, 32).expect("valid bounds"));

    assert!(!grid.global_shapes.contains(&shape_id));
    assert!(!grid.shape_cells[&shape_id].is_empty());
    assert!(
        grid.query_with_tolerance((144, 144), tolerance(1.0))
            .contains(&shape_id)
    );
}

#[test]
fn aggregate_membership_budget_routes_excess_shapes_to_global_candidates() {
    let mut frame = Frame::new();
    let first = frame.add_shape(filled_rect(10, 10, 10, 10));
    let second = frame.add_shape(filled_rect(74, 10, 10, 10));
    let third = frame.add_shape(filled_rect(138, 10, 10, 10));

    let grid = SpatialGrid::build_with_membership_limit(
        &TextMeasurer::default(),
        &frame,
        SPATIAL_GRID_CELL_SIZE,
        2,
    )
    .expect("spatial grid");

    assert_eq!(grid.indexed_memberships, 2);
    assert_membership_accounting(&grid);
    assert!(grid.shape_cells.contains_key(&first));
    assert!(grid.shape_cells.contains_key(&second));
    assert!(!grid.shape_cells.contains_key(&third));
    assert_eq!(grid.global_shapes, HashSet::from([third]));

    let candidates = grid.query_with_tolerance((15, 15), tolerance(1.0));
    assert!(candidates.contains(&first));
    assert!(candidates.contains(&third));
}

#[test]
fn aggregate_rejection_keeps_membership_storage_available_for_later_small_shape() {
    let mut frame = Frame::new();
    let first = frame.add_shape(filled_rect(10, 10, 10, 10));
    let mut grid = SpatialGrid::build_with_membership_limit(
        &TextMeasurer::default(),
        &frame,
        SPATIAL_GRID_CELL_SIZE,
        1,
    )
    .expect("spatial grid");
    let rejected = u64::MAX;
    let later_small = u64::MAX - 1;

    grid.add_shape_with_bounds(
        rejected,
        Rect::new(0, 0, SPATIAL_GRID_CELL_SIZE * 4_096, 1).expect("valid bounds"),
    );
    assert_eq!(grid.indexed_memberships, 1);
    assert!(!grid.shape_cells.contains_key(&rejected));
    assert!(grid.global_shapes.contains(&rejected));

    grid.remove_shape(first);
    grid.add_shape_with_bounds(later_small, Rect::new(0, 0, 1, 1).expect("valid bounds"));

    assert_eq!(grid.indexed_memberships, 1);
    assert!(grid.shape_cells.contains_key(&later_small));
    assert_membership_accounting(&grid);
}

#[test]
fn removing_indexed_shape_releases_aggregate_membership_budget() {
    let mut frame = Frame::new();
    let first = frame.add_shape(filled_rect(10, 10, 10, 10));
    let _second = frame.add_shape(filled_rect(74, 10, 10, 10));
    let third = frame.add_shape(filled_rect(138, 10, 10, 10));
    let third_bounds = frame
        .shape(third)
        .and_then(|shape| shape.bounding_box())
        .expect("third shape bounds");
    let mut grid = SpatialGrid::build_with_membership_limit(
        &TextMeasurer::default(),
        &frame,
        SPATIAL_GRID_CELL_SIZE,
        2,
    )
    .expect("spatial grid");

    grid.remove_shape(first);
    grid.remove_shape(third);
    grid.add_shape_with_bounds(third, third_bounds);

    assert_eq!(grid.indexed_memberships, 2);
    assert_membership_accounting(&grid);
    assert!(!grid.global_shapes.contains(&third));
    assert!(grid.shape_cells.contains_key(&third));
}

#[test]
fn reindexing_shape_beyond_remaining_budget_clears_old_cells_and_stays_queryable() {
    let mut frame = Frame::new();
    let first = frame.add_shape(filled_rect(10, 10, 10, 10));
    let _second = frame.add_shape(filled_rect(74, 10, 10, 10));
    let mut grid = SpatialGrid::build_with_membership_limit(
        &TextMeasurer::default(),
        &frame,
        SPATIAL_GRID_CELL_SIZE,
        2,
    )
    .expect("spatial grid");

    grid.remove_shape(first);
    grid.add_shape_with_bounds(first, Rect::new(0, 0, 128, 32).expect("valid bounds"));

    assert_eq!(grid.indexed_memberships, 1);
    assert_membership_accounting(&grid);
    assert!(!grid.cells.contains_key(&(0, 0)));
    assert!(!grid.shape_cells.contains_key(&first));
    assert!(grid.global_shapes.contains(&first));
    assert!(
        grid.query_with_tolerance((32, 16), tolerance(1.0))
            .contains(&first)
    );
}

#[test]
fn invalid_query_tolerances_cannot_enter_the_spatial_grid() {
    for tolerance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, f64::MAX] {
        assert!(HitTestTolerance::new(tolerance).is_none());
    }
}

#[test]
fn query_cell_budget_falls_back_before_radius_can_explode() {
    let (grid, [first, second, global]) = mixed_candidate_grid();
    let point = (6_400, 6_400);
    let within_budget = 30.0 * f64::from(SPATIAL_GRID_CELL_SIZE);
    let over_budget = 31.0 * f64::from(SPATIAL_GRID_CELL_SIZE);

    assert_eq!(
        candidate_set(grid.query_with_tolerance(point, tolerance(within_budget))),
        HashSet::from([global])
    );
    assert_eq!(
        candidate_set(grid.query_with_tolerance(point, tolerance(over_budget))),
        HashSet::from([first, second, global])
    );
}

#[test]
fn query_at_coordinate_limits_skips_out_of_range_neighbor_cells() {
    let min_id = 1;
    let max_id = 2;
    let grid = SpatialGrid {
        cell_size: 1,
        cells: HashMap::from([
            ((i32::MIN, i32::MIN), vec![min_id]),
            ((i32::MAX, i32::MAX), vec![max_id]),
        ]),
        shape_cells: HashMap::from([
            (min_id, vec![(i32::MIN, i32::MIN)]),
            (max_id, vec![(i32::MAX, i32::MAX)]),
        ]),
        global_shapes: HashSet::new(),
        indexed_memberships: 2,
        max_indexed_memberships: 2,
        shape_count: 2,
    };

    assert_eq!(
        candidate_set(grid.query_with_tolerance((i32::MIN, i32::MIN), tolerance(0.0))),
        HashSet::from([min_id])
    );
    assert_eq!(
        candidate_set(grid.query_with_tolerance((i32::MAX, i32::MAX), tolerance(0.0))),
        HashSet::from([max_id])
    );
}
