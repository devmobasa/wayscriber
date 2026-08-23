use super::*;
use crate::draw::{Color, Shape};

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

#[test]
fn oversized_shape_is_queried_without_per_cell_index_entries() {
    let mut frame = Frame::new();
    let shape_id = frame.add_shape(filled_rect(0, 0, 100_000, 100_000));

    let grid = SpatialGrid::build(&frame, SPATIAL_GRID_CELL_SIZE).expect("spatial grid");

    assert!(grid.cells.is_empty());
    assert!(grid.shape_cells.is_empty());
    assert_eq!(grid.global_shapes, HashSet::from([shape_id]));
    assert!(
        grid.query_with_tolerance((50_000, 50_000), 1.0)
            .contains(&shape_id)
    );
}

#[test]
fn cell_membership_uses_wide_arithmetic_near_coordinate_limits() {
    let bounds = Rect::new(i32::MAX - 31, i32::MAX - 31, 64, 64).expect("valid bounds");

    assert_eq!(
        SpatialGrid::compute_cell_membership(bounds, 1),
        CellMembership::Global
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
        SpatialGrid::compute_cell_membership(bounds, SPATIAL_GRID_CELL_SIZE),
        CellMembership::Global
    );
}

#[test]
fn oversized_shape_can_move_back_into_regular_cells() {
    let mut frame = Frame::new();
    let shape_id = frame.add_shape(filled_rect(0, 0, 100_000, 100_000));
    let mut grid = SpatialGrid::build(&frame, SPATIAL_GRID_CELL_SIZE).expect("spatial grid");

    grid.remove_shape(shape_id);
    grid.add_shape_with_bounds(shape_id, Rect::new(128, 128, 32, 32).expect("valid bounds"));

    assert!(!grid.global_shapes.contains(&shape_id));
    assert!(!grid.shape_cells[&shape_id].is_empty());
    assert!(
        grid.query_with_tolerance((144, 144), 1.0)
            .contains(&shape_id)
    );
}

#[test]
fn aggregate_membership_budget_routes_excess_shapes_to_global_candidates() {
    let mut frame = Frame::new();
    let first = frame.add_shape(filled_rect(10, 10, 10, 10));
    let second = frame.add_shape(filled_rect(74, 10, 10, 10));
    let third = frame.add_shape(filled_rect(138, 10, 10, 10));

    let grid = SpatialGrid::build_with_membership_limit(&frame, SPATIAL_GRID_CELL_SIZE, 2)
        .expect("spatial grid");

    assert_eq!(grid.indexed_memberships, 2);
    assert_membership_accounting(&grid);
    assert!(grid.shape_cells.contains_key(&first));
    assert!(grid.shape_cells.contains_key(&second));
    assert!(!grid.shape_cells.contains_key(&third));
    assert_eq!(grid.global_shapes, HashSet::from([third]));

    let candidates = grid.query_with_tolerance((15, 15), 1.0);
    assert!(candidates.contains(&first));
    assert!(candidates.contains(&third));
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
    let mut grid = SpatialGrid::build_with_membership_limit(&frame, SPATIAL_GRID_CELL_SIZE, 2)
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
    let mut grid = SpatialGrid::build_with_membership_limit(&frame, SPATIAL_GRID_CELL_SIZE, 2)
        .expect("spatial grid");

    grid.remove_shape(first);
    grid.add_shape_with_bounds(first, Rect::new(0, 0, 128, 32).expect("valid bounds"));

    assert_eq!(grid.indexed_memberships, 1);
    assert_membership_accounting(&grid);
    assert!(!grid.cells.contains_key(&(0, 0)));
    assert!(!grid.shape_cells.contains_key(&first));
    assert!(grid.global_shapes.contains(&first));
    assert!(grid.query_with_tolerance((32, 16), 1.0).contains(&first));
}
