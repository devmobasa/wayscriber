use super::*;
use crate::draw::{BLACK, Color, RED, WHITE};

/// Midpoint of the head's back edge, where the shaft meets the head.
fn head_base(geometry: &super::arrow::ArrowheadTriangle) -> (f64, f64) {
    (
        (geometry.left.0 + geometry.right.0) / 2.0,
        (geometry.left.1 + geometry.right.1) / 2.0,
    )
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[test]
fn arrowhead_triangle_caps_at_forty_percent_of_line_length() {
    // Line length = 10, requested head length = 100 -> capped at 40% = 4.
    let geometry = calculate_arrowhead_triangle_custom(10, 10, 0, 10, 1.0, 100.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    assert!((distance(geometry.tip, head_base(&geometry)) - 4.0).abs() < f64::EPSILON);
}

#[test]
fn arrowhead_triangle_handles_degenerate_lines() {
    let geometry = calculate_arrowhead_triangle_custom(5, 5, 5, 5, 2.0, 15.0, 45.0);
    assert!(geometry.is_none());
}

#[test]
fn ellipse_bounds_compute_center_and_radii() {
    let (cx, cy, rx, ry) = ellipse_bounds(0, 0, 10, 4);
    assert_eq!((cx, cy, rx, ry), (5, 2, 5, 2));
}

#[test]
fn name_color_mappings_resolve_known_colors() {
    assert_eq!(name_to_color("white").unwrap(), WHITE);
    assert!(name_to_color("chartreuse").is_none());
}

#[test]
fn color_to_name_matches_known_colors() {
    assert_eq!(color_to_name(&RED), "Red");
    assert_eq!(color_to_name(&BLACK), "Black");
    assert_eq!(
        color_to_name(&Color {
            r: 0.42,
            g: 0.42,
            b: 0.42,
            a: 1.0
        }),
        "Custom"
    );
}

#[test]
fn rect_contains_is_min_inclusive_max_exclusive() {
    let rect = Rect::new(0, 0, 10, 10).unwrap();
    assert!(rect.contains(0, 0));
    assert!(rect.contains(9, 9));
    assert!(!rect.contains(10, 10));
    assert!(!rect.contains(-1, 0));
}

#[test]
fn rect_inflated_returns_none_when_degenerate() {
    let rect = Rect::new(0, 0, 2, 2).unwrap();
    assert!(rect.inflated(-2).is_none());
}

#[test]
fn arrowhead_triangle_scales_the_head_with_stroke_width() {
    // A stub head on a thick stroke reads as a line with a nub, so the head
    // grows with the stroke: 10px thick -> 30px head, past `arrow_length`.
    // The line is long enough that the 40%-of-length cap does not bite.
    let geometry = calculate_arrowhead_triangle_custom(400, 0, 0, 0, 10.0, 1.0, 24.0)
        .expect("non-degenerate line should yield geometry");
    assert!((distance(geometry.tip, head_base(&geometry)) - 30.0).abs() < 1e-9);
}

#[test]
fn arrow_length_is_the_floor_for_thin_strokes() {
    // Below the scaled size, `arrow.length` still decides, so hairline strokes
    // keep a visible head.
    let geometry = calculate_arrowhead_triangle_custom(400, 0, 0, 0, 1.0, 20.0, 24.0)
        .expect("non-degenerate line should yield geometry");
    assert!((distance(geometry.tip, head_base(&geometry)) - 20.0).abs() < 1e-9);
}

#[test]
fn arrowhead_triangle_uses_thickness_floor_for_half_base() {
    let geometry = calculate_arrowhead_triangle_custom(100, 0, 0, 0, 10.0, 5.0, 1.0)
        .expect("non-degenerate line should yield geometry");
    let half_base = (geometry.left.1 - geometry.right.1).abs() / 2.0;
    assert!((half_base - 6.0).abs() < 1e-9);
}

#[test]
fn arrowhead_back_edge_is_perpendicular_to_the_shaft() {
    let geometry = calculate_arrowhead_triangle_custom(50, 50, 0, 0, 3.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    // The base midpoint must sit on the tip -> tail axis, so the head is not skewed.
    let base = head_base(&geometry);
    let axis: (f64, f64) = (0.0 - 50.0, 0.0 - 50.0);
    let axis_len = (axis.0 * axis.0 + axis.1 * axis.1).sqrt();
    let to_base = (base.0 - 50.0, base.1 - 50.0);
    let cross = (axis.0 * to_base.1 - axis.1 * to_base.0) / axis_len;
    assert!(
        cross.abs() < 1e-9,
        "base midpoint drifted {cross} off the shaft axis"
    );
}

/// Signed sideways offset of `point` from the tip -> tail axis.
fn perpendicular_offset(point: (f64, f64), tip: (f64, f64), tail: (f64, f64)) -> f64 {
    let axis = (tail.0 - tip.0, tail.1 - tip.1);
    let axis_len = (axis.0 * axis.0 + axis.1 * axis.1).sqrt();
    let to_point = (point.0 - tip.0, point.1 - tip.1);
    (axis.0 * to_point.1 - axis.1 * to_point.0) / axis_len
}

/// How far `point` sits back from the tip, measured along the tip -> tail axis.
fn axial_offset(point: (f64, f64), tip: (f64, f64), tail: (f64, f64)) -> f64 {
    let axis = (tail.0 - tip.0, tail.1 - tip.1);
    let axis_len = (axis.0 * axis.0 + axis.1 * axis.1).sqrt();
    let to_point = (point.0 - tip.0, point.1 - tip.1);
    (axis.0 * to_point.0 + axis.1 * to_point.1) / axis_len
}

#[test]
fn arrow_outline_handles_degenerate_lines() {
    assert!(calculate_arrow_outline(5, 5, 5, 5, 2.0, 15.0, 45.0).is_none());
}

#[test]
fn arrow_outline_head_matches_the_head_triangle() {
    // Render (outline) and hit-test/bounds (triangle) must not drift apart.
    let triangle = calculate_arrowhead_triangle_custom(90, 40, 10, 70, 6.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    let outline = calculate_arrow_outline(90, 40, 10, 70, 6.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");

    assert!(distance(outline.points[2], triangle.left) < 1e-9);
    assert!(distance(outline.points[3], triangle.tip) < 1e-9);
    assert!(distance(outline.points[4], triangle.right) < 1e-9);
}

#[test]
fn arrow_outline_bevels_the_rear_edge_into_the_shaft() {
    // The rear of the head must not be one straight line across: the shoulders
    // sit forward of the base so each rear edge bevels inward to the shaft.
    let tip = (90.0, 40.0);
    let tail = (10.0, 70.0);
    let triangle = calculate_arrowhead_triangle_custom(90, 40, 10, 70, 6.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    let outline = calculate_arrow_outline(90, 40, 10, 70, 6.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");

    let base_along = axial_offset(triangle.left, tip, tail);
    for shoulder in [outline.points[1], outline.points[5]] {
        let along = axial_offset(shoulder, tip, tail);
        assert!(
            along < base_along - 1e-9,
            "shoulder sits {along} from the tip, level with the base at \
             {base_along}: the rear edge would run straight across"
        );
    }
}

#[test]
fn arrow_outline_bevel_stays_shallow_enough_to_read_as_one_arrow() {
    // Deepening the bevel past roughly a fifth of the head opens a visible gap
    // between barb and shaft, and the head stops reading as part of the arrow.
    let tip = (90.0, 40.0);
    let tail = (10.0, 70.0);
    let triangle = calculate_arrowhead_triangle_custom(90, 40, 10, 70, 6.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    let outline = calculate_arrow_outline(90, 40, 10, 70, 6.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");

    let base_along = axial_offset(triangle.left, tip, tail);
    for shoulder in [outline.points[1], outline.points[5]] {
        let cut = base_along - axial_offset(shoulder, tip, tail);
        assert!(
            cut <= base_along * 0.2,
            "bevel cuts {cut} back from a {base_along}-long head, over the fifth \
             that keeps the head joined to the shaft"
        );
    }
}

#[test]
fn arrow_outline_shoulders_stay_inside_the_head_triangle() {
    // Hit-testing and dirty-region bounds use the head triangle alone, so the
    // shoulders must not poke past it. A thick stroke on a narrow angle is the
    // worst case: the head sits at its floor width and the shaft is widest.
    let tip = (100.0, 0.0);
    let tail = (0.0, 0.0);
    let outline = calculate_arrow_outline(100, 0, 0, 0, 20.0, 5.0, 15.0)
        .expect("non-degenerate line should yield geometry");
    let triangle = calculate_arrowhead_triangle_custom(100, 0, 0, 0, 20.0, 5.0, 15.0)
        .expect("non-degenerate line should yield geometry");

    let head_half = perpendicular_offset(triangle.left, tip, tail).abs();
    let head_length = axial_offset(triangle.left, tip, tail);
    for shoulder in [outline.points[1], outline.points[5]] {
        let along = axial_offset(shoulder, tip, tail);
        let across = perpendicular_offset(shoulder, tip, tail).abs();
        // The triangle narrows linearly from the base toward the tip.
        let allowed = head_half * along / head_length;
        assert!(
            across <= allowed + 1e-9,
            "shoulder is {across} off-axis where the head only allows {allowed}"
        );
    }
}

#[test]
fn arrow_outline_keeps_full_shaft_width_on_thick_strokes() {
    // Clamping the shoulders to the narrowed head must not thin the shaft below
    // the requested stroke.
    let tip = (100.0, 0.0);
    let tail = (0.0, 0.0);
    let outline = calculate_arrow_outline(100, 0, 0, 0, 20.0, 5.0, 15.0)
        .expect("non-degenerate line should yield geometry");
    let shoulder_half = perpendicular_offset(outline.points[1], tip, tail).abs();
    assert!(
        (shoulder_half - 10.0).abs() < 1e-9,
        "shaft narrowed to {shoulder_half} instead of half the 20.0 stroke"
    );
}

#[test]
fn arrow_outline_tapers_from_tail_to_shoulder() {
    let tip = (100.0, 0.0);
    let tail = (0.0, 0.0);
    let outline = calculate_arrow_outline(100, 0, 0, 0, 10.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");

    let tail_half = perpendicular_offset(outline.points[0], tip, tail).abs();
    let shoulder_half = perpendicular_offset(outline.points[1], tip, tail).abs();
    let head_half = perpendicular_offset(outline.points[2], tip, tail).abs();

    assert!(
        (shoulder_half - 5.0).abs() < 1e-9,
        "shoulder should be half the stroke thickness, got {shoulder_half}"
    );
    assert!(
        tail_half < shoulder_half,
        "tail ({tail_half}) must be narrower than the shoulder ({shoulder_half})"
    );
    assert!(
        head_half > shoulder_half,
        "head ({head_half}) must be wider than the shoulder ({shoulder_half})"
    );
}

#[test]
fn arrow_outline_is_symmetric_about_the_shaft_axis() {
    let tip = (90.0, 40.0);
    let tail = (10.0, 70.0);
    let outline = calculate_arrow_outline(90, 40, 10, 70, 6.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");

    // Points mirror in pairs around the tip at index 3.
    for (left, right) in [(0, 6), (1, 5), (2, 4)] {
        let a = perpendicular_offset(outline.points[left], tip, tail);
        let b = perpendicular_offset(outline.points[right], tip, tail);
        assert!(
            (a + b).abs() < 1e-9,
            "points {left}/{right} are not mirrored: {a} vs {b}"
        );
    }
    assert!(perpendicular_offset(outline.points[3], tip, tail).abs() < 1e-9);
}

#[test]
fn arrow_outline_keeps_a_visible_tail_for_thin_strokes() {
    // A 1px stroke tapered by ratio alone would leave a sub-pixel tail.
    let outline = calculate_arrow_outline(100, 0, 0, 0, 1.0, 20.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    let tail_half = perpendicular_offset(outline.points[0], (100.0, 0.0), (0.0, 0.0)).abs();
    let shoulder_half = perpendicular_offset(outline.points[1], (100.0, 0.0), (0.0, 0.0)).abs();
    assert!(
        tail_half >= 0.5,
        "tail collapsed to {tail_half}, thin arrows would fade out"
    );
    assert!(
        tail_half <= shoulder_half,
        "taper inverted: tail {tail_half} wider than shoulder {shoulder_half}"
    );
}
