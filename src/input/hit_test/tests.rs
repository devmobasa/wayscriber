use super::*;
use crate::draw::{
    ArrowLabel, ArrowStyle, BLACK, DrawnShape, EmbeddedImage, EraserBrush, EraserKind,
    FontDescriptor, PolygonKind, Shape, StepMarkerLabel,
};

#[test]
fn rectangle_hit_testing_widens_extreme_persisted_extents() {
    assert!(shapes::rect_outline_hit(
        i32::MIN,
        0,
        i32::MIN,
        20,
        1.0,
        (i32::MIN, 10),
        1.0,
    ));
    assert!(shapes::rect_fill_hit(
        i32::MAX,
        0,
        i32::MAX,
        20,
        (i32::MAX, 10),
    ));
}

#[test]
fn compute_hit_bounds_inflates_bounds_for_tolerance() {
    let drawn = DrawnShape::with_metadata(
        1,
        Shape::Rect {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
            fill: false,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

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
    let eraser = DrawnShape::with_metadata(
        2,
        Shape::EraserStroke {
            points: vec![(0, 0), (10, 10)],
            brush: EraserBrush {
                size: 8.0,
                kind: EraserKind::Circle,
            },
        },
        0,
        false,
    );

    assert!(
        compute_hit_bounds(&eraser, 5.0).is_none(),
        "eraser strokes should not participate in hit bounds"
    );
}

#[test]
fn invalid_tolerances_fail_closed() {
    let drawn = DrawnShape::with_metadata(
        3,
        Shape::Line {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 0,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

    for tolerance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, f64::MAX] {
        assert!(compute_hit_bounds(&drawn, tolerance).is_none());
        assert!(!hit_test(&drawn, (10, 0), tolerance));
        assert!(!hit_test_for_point_targeting(&drawn, (10, 0), tolerance));
    }
}

#[test]
fn rect_hit_handles_degenerate_dimensions() {
    let rect = DrawnShape::with_metadata(
        1,
        Shape::Rect {
            x: 10,
            y: 10,
            w: 0,
            h: 20,
            fill: false,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

    assert!(hit_test(&rect, (10, 10), 3.0));
    assert!(!hit_test(&rect, (5, 5), 2.0));
}

#[test]
fn ellipse_hit_handles_zero_radius() {
    let ellipse = DrawnShape::with_metadata(
        2,
        Shape::Ellipse {
            cx: 50,
            cy: 80,
            rx: 0,
            ry: 0,
            fill: false,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

    assert!(hit_test(&ellipse, (50, 80), 2.0));
    assert!(!hit_test(&ellipse, (60, 90), 1.0));
}

#[test]
fn polygon_hit_tests_closed_outline_only() {
    let polygon = DrawnShape::with_metadata(
        3,
        Shape::Polygon {
            kind: PolygonKind::Triangle,
            points: vec![(10, 10), (40, 10), (25, 40)],
            fill: true,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

    assert!(hit_test(&polygon, (25, 10), 1.0));
    assert!(
        !hit_test(&polygon, (25, 22), 1.0),
        "generic hit testing remains outline-only for filled polygon interiors"
    );
}

#[test]
fn point_targeting_hits_filled_rect_and_ellipse_interiors() {
    let rect = DrawnShape::with_metadata(
        3,
        Shape::Rect {
            x: 10,
            y: 10,
            w: 40,
            h: 30,
            fill: true,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );
    let ellipse = DrawnShape::with_metadata(
        4,
        Shape::Ellipse {
            cx: 80,
            cy: 70,
            rx: 20,
            ry: 12,
            fill: true,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

    assert!(!hit_test(&rect, (30, 25), 1.0));
    assert!(hit_test_for_point_targeting(&rect, (30, 25), 1.0));
    assert!(!hit_test(&ellipse, (80, 70), 1.0));
    assert!(hit_test_for_point_targeting(&ellipse, (80, 70), 1.0));
}

#[test]
fn point_targeting_hits_filled_polygon_interior() {
    let polygon = DrawnShape::with_metadata(
        3,
        Shape::Polygon {
            kind: PolygonKind::Triangle,
            points: vec![(10, 10), (40, 10), (25, 40)],
            fill: true,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

    assert!(
        hit_test_for_point_targeting(&polygon, (25, 22), 1.0),
        "filled polygon interiors should be directly targetable"
    );
}

#[test]
fn invalid_polygon_hit_test_is_false() {
    let polygon = DrawnShape::with_metadata(
        4,
        Shape::Polygon {
            kind: PolygonKind::Freeform,
            points: vec![(10, 10), (10, 10), (40, 10)],
            fill: false,
            color: BLACK,
            thick: 2.0,
        },
        0,
        false,
    );

    assert!(!hit_test(&polygon, (10, 10), 10.0));
}

#[test]
fn arrowhead_hit_detects_point_near_tip_and_rejects_distant_point() {
    // Arrow pointing upwards from tail at (0, -20) to tip at (0, 0).
    let tip = (0, 0);
    let tail = (0, -20);
    let skeleton = crate::util::calculate_arrow_skeleton(
        tip.0,
        tip.1,
        tail.0,
        tail.1,
        2.0,
        10.0,
        30.0,
        ArrowStyle::Standard,
        0.0,
    )
    .expect("non-degenerate arrow should yield geometry");

    assert!(
        shapes::arrowhead_triangle_hit(&skeleton.head, tip, 0.5),
        "tip point should be inside arrowhead"
    );

    assert!(
        !shapes::arrowhead_triangle_hit(&skeleton.head, (50, 50), 0.5),
        "faraway point should not be inside arrowhead even with tolerance"
    );
}

fn arrow_shape(style: ArrowStyle, bend: f64) -> DrawnShape {
    // Runs right along y = 100 with the head at (400, 100).
    DrawnShape::with_metadata(
        7,
        Shape::Arrow {
            x1: 0,
            y1: 100,
            x2: 400,
            y2: 100,
            color: BLACK,
            thick: 4.0,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style,
            bend,
            label: None,
        },
        0,
        false,
    )
}

#[test]
fn curved_arrow_is_grabbed_on_its_arc_and_not_on_the_chord_it_bypasses() {
    // The point of a curved arrow is that it routes around whatever sits on the
    // chord. Testing the chord instead would select it by clicking the thing it
    // was drawn to avoid.
    let curved = arrow_shape(ArrowStyle::Curved, 0.5);

    // Bend 0.5 over a 400px chord puts the arc's midpoint 100px above it.
    assert!(
        hit_test(&curved, (200, 0), 2.0),
        "the arc's bulge should be grabbable"
    );
    assert!(
        !hit_test(&curved, (200, 100), 2.0),
        "the chord the arrow routes around should not be a hit target"
    );
}

#[test]
fn double_arrow_is_grabbed_by_either_head() {
    let double = arrow_shape(ArrowStyle::Double, 0.0);
    let standard = arrow_shape(ArrowStyle::Standard, 0.0);

    // The head is 20px long with a half-base of 20*tan(30 deg) ~ 11.5, so at
    // 10px in from the tail it spans 5.8px either side of the chord. A point
    // 5px off the chord there is inside the tail head and well outside the
    // 2px-radius shaft a standard arrow tapers to.
    let on_tail_barb = (10, 95);
    assert!(
        hit_test(&double, on_tail_barb, 0.5),
        "the second head should be a hit target"
    );
    assert!(
        !hit_test(&standard, on_tail_barb, 0.5),
        "test setup: a standard arrow has no barb there to hit"
    );
}

#[test]
fn arrow_label_hit_detects_label_bounds() {
    let font = FontDescriptor::default();
    let label = ArrowLabel {
        value: 12,
        size: 12.0,
        font_descriptor: font.clone(),
    };
    let drawn = DrawnShape::with_metadata(
        3,
        Shape::Arrow {
            x1: 0,
            y1: 0,
            x2: 100,
            y2: 0,
            color: BLACK,
            thick: 2.0,
            arrow_length: 10.0,
            arrow_angle: 30.0,
            head_at_end: true,
            style: ArrowStyle::Standard,
            bend: 0.0,
            label: Some(label),
        },
        0,
        false,
    );

    let label_text = "12";
    let layout =
        crate::draw::shape::arrow_label_layout(100, 0, 0, 0, 2.0, 0.0, label_text, 12.0, &font)
            .expect("label layout should exist");
    let hit_point = (
        layout.bounds.x + layout.bounds.width / 2,
        layout.bounds.y + layout.bounds.height / 2,
    );

    assert!(
        hit_test(&drawn, hit_point, 0.1),
        "label center should be hittable"
    );
    assert!(
        !hit_test(&drawn, (hit_point.0, hit_point.1 + 200), 0.1),
        "distant point should not hit label"
    );
}

#[test]
fn step_marker_hit_detects_center_and_rejects_outside_point() {
    let font = FontDescriptor::default();
    let label = StepMarkerLabel {
        value: 7,
        size: 16.0,
        font_descriptor: font,
    };
    let drawn = DrawnShape::with_metadata(
        4,
        Shape::StepMarker {
            x: 50,
            y: 60,
            color: BLACK,
            label,
        },
        0,
        false,
    );

    assert!(
        hit_test(&drawn, (50, 60), 0.1),
        "center point should hit step marker"
    );

    let Shape::StepMarker { label, .. } = &drawn.shape else {
        panic!("expected step marker shape");
    };
    let radius =
        crate::draw::shape::step_marker_radius(label.value, label.size, &label.font_descriptor);
    let outline = crate::draw::shape::step_marker_outline_thickness(label.size);
    let outside_x = 50 + (radius + outline / 2.0).ceil() as i32 + 2;
    assert!(
        !hit_test(&drawn, (outside_x, 60), 0.1),
        "point outside radius should miss step marker"
    );
}

#[test]
fn image_hit_test_uses_display_rectangle() {
    let drawn = DrawnShape::with_metadata(
        5,
        Shape::Image {
            x: 10,
            y: 20,
            w: 40,
            h: 30,
            data: EmbeddedImage {
                mime_type: "image/png".to_string(),
                width: 4,
                height: 3,
                bytes: vec![1, 2, 3].into(),
            },
        },
        0,
        false,
    );

    assert!(hit_test(&drawn, (20, 30), 0.0));
    assert!(!hit_test(&drawn, (60, 60), 0.0));
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

/// A rectangle stored with negative extents paints normalized, so it has to
/// be selectable along the outline it actually draws. Hit testing it as a
/// bare point at its origin left such a shape visible but unclickable.
#[test]
fn rect_outline_hit_normalizes_negative_extents() {
    // The same rectangle expressed from each opposite corner.
    for point in [(10, 20), (40, 60), (25, 20), (10, 40), (40, 40)] {
        assert_eq!(
            shapes::rect_outline_hit(10, 20, 30, 40, 2.0, point, 3.0),
            shapes::rect_outline_hit(40, 60, -30, -40, 2.0, point, 3.0),
            "{point:?} must hit both spellings of the same rectangle"
        );
        assert!(
            shapes::rect_outline_hit(40, 60, -30, -40, 2.0, point, 3.0),
            "{point:?} lies on the painted outline"
        );
    }

    // A point well inside the rectangle still misses the outline.
    assert!(!shapes::rect_outline_hit(
        40,
        60,
        -30,
        -40,
        2.0,
        (25, 40),
        3.0
    ));
}

/// A rectangle with no extent at all is still a point.
#[test]
fn rect_outline_hit_treats_a_zero_rect_as_a_point() {
    assert!(shapes::rect_outline_hit(10, 20, 0, 0, 2.0, (11, 21), 3.0));
    assert!(!shapes::rect_outline_hit(10, 20, 0, 0, 2.0, (30, 40), 3.0));
}

#[test]
fn pressure_stroke_hit_includes_one_point_stylus_dots() {
    let dot = DrawnShape::with_metadata(
        1,
        Shape::FreehandPressure {
            points: vec![(50, 50, 20.0)],
            color: BLACK,
        },
        0,
        false,
    );

    assert!(
        hit_test(&dot, (50, 50), 1.0),
        "a one-point pressure stroke paints a cap circle and must be selectable"
    );
    assert!(hit_test(&dot, (55, 50), 1.0));
    assert!(!hit_test(&dot, (80, 80), 1.0));
}

/// A labelled arrow along y = 100 from (0, 100) to (400, 100).
fn labelled_arrow_shape(style: ArrowStyle, head_at_end: bool) -> DrawnShape {
    DrawnShape::with_metadata(
        1,
        Shape::Arrow {
            x1: 0,
            y1: 100,
            x2: 400,
            y2: 100,
            color: BLACK,
            thick: 4.0,
            arrow_length: 20.0,
            arrow_angle: 30.0,
            head_at_end,
            style,
            bend: 0.0,
            label: Some(ArrowLabel {
                value: 7,
                size: 12.0,
                font_descriptor: FontDescriptor::default(),
            }),
        },
        0,
        false,
    )
}

#[test]
fn a_double_arrow_label_is_grabbable_from_the_same_place_either_way() {
    // head_at_end has no effect on a Double arrow — the outline is the same
    // polygon either way, and docs/CONFIG.md says so. The hit area has to
    // follow, or the number is grabbable where it is not painted on exactly
    // one of the two readings.
    let font = FontDescriptor::default();
    let layout =
        crate::draw::shape::arrow_label_layout(400, 100, 0, 100, 4.0, 0.0, "7", 12.0, &font)
            .expect("label layout should exist");
    let center = (
        layout.bounds.x + layout.bounds.width / 2,
        layout.bounds.y + layout.bounds.height / 2,
    );

    for head_at_end in [true, false] {
        assert!(
            hit_test(
                &labelled_arrow_shape(ArrowStyle::Double, head_at_end),
                center,
                0.1
            ),
            "double arrow label was not grabbable with head_at_end = {head_at_end}"
        );
    }
}
