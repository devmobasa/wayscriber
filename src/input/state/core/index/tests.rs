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
