use super::*;
use crate::draw::{BLACK, DrawnShape, EraserBrush, EraserKind, Shape};

#[test]
fn compute_hit_bounds_inflates_bounds_for_tolerance() {
    let drawn = DrawnShape {
        id: 1,
        shape: Shape::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            fill: false,
            color: BLACK,
            thick: 2.0,
        },
        created_at: 0,
        locked: false,
    };

    let base = drawn
        .shape
        .bounding_box()
        .expect("rect should have base bounds");
    let expanded =
        compute_hit_bounds(&drawn, 4.2).expect("compute_hit_bounds should expand rectangle");

    assert!(expanded.x <= base.x);
    assert!(expanded.y <= base.y);
    assert!(expanded.width >= base.width);
    assert!(expanded.height >= base.height);
}

#[test]
fn compute_hit_bounds_ignores_eraser_strokes() {
    let eraser = DrawnShape {
        id: 2,
        shape: Shape::EraserStroke {
            points: vec![(0, 0), (10, 10)],
            brush: EraserBrush {
                size: 8.0,
                kind: EraserKind::Circle,
            },
        },
        created_at: 0,
        locked: false,
    };

    assert!(
        compute_hit_bounds(&eraser, 5.0).is_none(),
        "eraser strokes should not participate in hit bounds"
    );
}

#[test]
fn rect_hit_handles_degenerate_dimensions() {
    let rect = DrawnShape {
        id: 1,
        shape: Shape::Rect {
            x: 10,
            y: 10,
            w: 0,
            h: 20,
            fill: false,
            color: BLACK,
            thick: 2.0,
        },
        created_at: 0,
        locked: false,
    };

    assert!(hit_test(&rect, (10, 10), 3.0));
    assert!(!hit_test(&rect, (5, 5), 2.0));
}

#[test]
fn ellipse_hit_handles_zero_radius() {
    let ellipse = DrawnShape {
        id: 2,
        shape: Shape::Ellipse {
            cx: 50,
            cy: 80,
            rx: 0,
            ry: 0,
            fill: false,
            color: BLACK,
            thick: 2.0,
        },
        created_at: 0,
        locked: false,
    };

    assert!(hit_test(&ellipse, (50, 80), 2.0));
    assert!(!hit_test(&ellipse, (60, 90), 1.0));
}

#[test]
fn arrowhead_hit_detects_point_near_tip_and_rejects_distant_point() {
    // Arrow pointing upwards from tail at (0, -20) to tip at (0, 0).
    let tip = (0, 0);
    let tail = (0, -20);

    assert!(
        shapes::arrowhead_hit(tip.0, tip.1, tail.0, tail.1, 10.0, 30.0, tip, 0.5),
        "tip point should be inside arrowhead"
    );

    assert!(
        !shapes::arrowhead_hit(tip.0, tip.1, tail.0, tail.1, 10.0, 30.0, (50, 50), 0.5),
        "faraway point should not be inside arrowhead even with tolerance"
    );
}

#[test]
fn distance_point_to_segment_matches_point_distance_for_zero_length_segment() {
    let start = (10, 10);
    let point = (13, 14);

    let seg_dist = geometry::distance_point_to_segment(point, start, start);
    let direct = geometry::distance_point_to_point(start, point);

    assert!(
        (seg_dist - direct).abs() < 1e-6,
        "distance to zero-length segment should equal point distance"
    );
}
