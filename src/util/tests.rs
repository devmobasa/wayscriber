use super::arrow::calculate_arrow_outline;
use super::*;
use crate::draw::{ArrowStyle, BLACK, Color, RED, WHITE};

/// The head triangle of a straight arrow, through the same skeleton the
/// renderer, bounds, and hit-testing all read.
#[allow(clippy::too_many_arguments)]
fn head_triangle(
    tip_x: i32,
    tip_y: i32,
    tail_x: i32,
    tail_y: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
) -> Option<super::arrow::ArrowheadTriangle> {
    calculate_arrow_skeleton(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
        ArrowStyle::Standard,
        0.0,
    )
    .map(|skeleton| skeleton.head)
}

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
    let geometry = head_triangle(10, 10, 0, 10, 1.0, 100.0, 30.0)
        .expect("non-degenerate line should yield geometry");
    assert!((distance(geometry.tip, head_base(&geometry)) - 4.0).abs() < f64::EPSILON);
}

#[test]
fn arrowhead_triangle_handles_degenerate_lines() {
    let geometry = head_triangle(5, 5, 5, 5, 2.0, 15.0, 45.0);
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
    let geometry = head_triangle(400, 0, 0, 0, 10.0, 1.0, 24.0)
        .expect("non-degenerate line should yield geometry");
    assert!((distance(geometry.tip, head_base(&geometry)) - 30.0).abs() < 1e-9);
}

#[test]
fn arrow_length_is_the_floor_for_thin_strokes() {
    // Below the scaled size, `arrow.length` still decides, so hairline strokes
    // keep a visible head.
    let geometry = head_triangle(400, 0, 0, 0, 1.0, 20.0, 24.0)
        .expect("non-degenerate line should yield geometry");
    assert!((distance(geometry.tip, head_base(&geometry)) - 20.0).abs() < 1e-9);
}

#[test]
fn arrowhead_triangle_uses_thickness_floor_for_half_base() {
    let geometry = head_triangle(100, 0, 0, 0, 10.0, 5.0, 1.0)
        .expect("non-degenerate line should yield geometry");
    let half_base = (geometry.left.1 - geometry.right.1).abs() / 2.0;
    assert!((half_base - 6.0).abs() < 1e-9);
}

#[test]
fn arrowhead_back_edge_is_perpendicular_to_the_shaft() {
    let geometry = head_triangle(50, 50, 0, 0, 3.0, 20.0, 30.0)
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
    let triangle = head_triangle(90, 40, 10, 70, 6.0, 20.0, 30.0)
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
    let triangle = head_triangle(90, 40, 10, 70, 6.0, 20.0, 30.0)
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
    let triangle = head_triangle(90, 40, 10, 70, 6.0, 20.0, 30.0)
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
    let triangle = head_triangle(100, 0, 0, 0, 20.0, 5.0, 15.0)
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

// --- Arrow styles ---------------------------------------------------------

/// Every style's outline, for one horizontal arrow pointing right.
fn styled_outline(style: ArrowStyle, bend: f64) -> Vec<(f64, f64)> {
    calculate_arrow_outline_styled(400, 100, 0, 100, 8.0, 20.0, 30.0, style, bend)
        .expect("non-degenerate arrow should yield an outline")
}

fn styled_skeleton(style: ArrowStyle, bend: f64) -> super::arrow::ArrowSkeleton {
    calculate_arrow_skeleton(400, 100, 0, 100, 8.0, 20.0, 30.0, style, bend)
        .expect("non-degenerate arrow should yield a skeleton")
}

#[test]
fn standard_styled_outline_is_the_historical_outline_unchanged() {
    // The back-compat guarantee at the geometry level: sessions drawn before
    // styles existed have to paint the same pixels. Break it by giving
    // `Standard` its own point list and this fails on the first coordinate.
    let historical = calculate_arrow_outline(400, 100, 0, 100, 8.0, 20.0, 30.0)
        .expect("non-degenerate arrow should yield geometry");
    let styled = styled_outline(ArrowStyle::Standard, 0.0);

    assert_eq!(styled.len(), historical.points.len());
    for (index, (styled_point, historical_point)) in
        styled.iter().zip(historical.points.iter()).enumerate()
    {
        assert_eq!(
            styled_point, historical_point,
            "point {index} drifted from the historical outline"
        );
    }
}

#[test]
fn every_style_produces_a_distinct_outline() {
    let outlines: Vec<Vec<(f64, f64)>> = ArrowStyle::ALL
        .iter()
        .map(|style| styled_outline(*style, 0.3))
        .collect();

    for (i, first) in outlines.iter().enumerate() {
        for (j, second) in outlines.iter().enumerate().skip(i + 1) {
            assert_ne!(
                first,
                second,
                "styles {:?} and {:?} draw the same outline",
                ArrowStyle::ALL[i],
                ArrowStyle::ALL[j]
            );
        }
    }
}

#[test]
fn pointy_notches_the_head_rear_deeper_than_standard() {
    // Both styles keep the same barbs; what makes a dart is how far forward the
    // rear notch is pulled. Measured from the tip along the shaft.
    let standard = styled_outline(ArrowStyle::Standard, 0.0);
    let pointy = styled_outline(ArrowStyle::Pointy, 0.0);

    // Index 1 is the shaft/head join on the first side, and the arrow points
    // left-to-right along y = 100 with the tip at x = 400.
    let standard_notch_x = standard[1].0;
    let pointy_notch_x = pointy[1].0;
    assert!(
        pointy_notch_x > standard_notch_x,
        "pointy notch at {pointy_notch_x} is not forward of standard's {standard_notch_x}"
    );
    // The barbs themselves must not move, or `arrow_angle` would mean something
    // different per style.
    assert_eq!(standard[2], pointy[2], "pointy moved the head barb");
    assert_eq!(standard[3], pointy[3], "pointy moved the tip");
}

#[test]
fn double_puts_a_head_at_the_tail_and_drops_the_taper() {
    let skeleton = styled_skeleton(ArrowStyle::Double, 0.0);
    let tail_head = skeleton
        .tail_head
        .expect("double-ended arrows carry a second head");
    assert_eq!(
        tail_head.tip,
        (0.0, 100.0),
        "tail head should point at the tail"
    );

    // The barbs of the tail head are as wide as the tip head's.
    let tip_half = (skeleton.head.left.1 - skeleton.head.right.1).abs();
    let tail_half = (tail_head.left.1 - tail_head.right.1).abs();
    assert!(
        (tip_half - tail_half).abs() < 1e-9,
        "heads disagree on width: tip {tip_half} vs tail {tail_half}"
    );

    // A tapered shaft would make the tail end narrower than the shoulder; a
    // double-ended one keeps parallel sides.
    let outline = styled_outline(ArrowStyle::Double, 0.0);
    let tail_notch_half = (outline[2].1 - 100.0).abs();
    let tip_notch_half = (outline[3].1 - 100.0).abs();
    assert!(
        (tail_notch_half - tip_notch_half).abs() < 1e-9,
        "double shaft tapered: {tail_notch_half} at the tail vs {tip_notch_half} at the head"
    );
}

#[test]
fn other_styles_ignore_bend() {
    for style in [ArrowStyle::Standard, ArrowStyle::Pointy, ArrowStyle::Double] {
        assert_eq!(
            styled_outline(style, 0.0),
            styled_outline(style, 0.8),
            "{style:?} changed shape for a bend it does not draw"
        );
    }
}

#[test]
fn curved_spine_bulges_off_the_chord_and_follows_the_bend_sign() {
    // The arrow runs right-to-left along y = 100 (tip at x = 400, tail at 0),
    // so a positive bend bulges to the left of travel, which is up on screen.
    let positive = styled_skeleton(ArrowStyle::Curved, 0.4);
    let negative = styled_skeleton(ArrowStyle::Curved, -0.4);

    let lowest_y = |skeleton: &super::arrow::ArrowSkeleton| {
        skeleton
            .spine
            .points()
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min)
    };
    let highest_y = |skeleton: &super::arrow::ArrowSkeleton| {
        skeleton
            .spine
            .points()
            .iter()
            .map(|point| point.1)
            .fold(f64::NEG_INFINITY, f64::max)
    };

    assert!(
        lowest_y(&positive) < 100.0 - 10.0,
        "positive bend did not bulge above the chord: min y {}",
        lowest_y(&positive)
    );
    assert!(
        highest_y(&negative) > 100.0 + 10.0,
        "negative bend did not bulge below the chord: max y {}",
        highest_y(&negative)
    );
}

#[test]
fn curved_with_zero_bend_stays_on_the_chord() {
    // Choosing Curved and then flattening it must not leave a wobble behind.
    let skeleton = styled_skeleton(ArrowStyle::Curved, 0.0);
    for point in skeleton.spine.points() {
        assert!(
            (point.1 - 100.0).abs() < 1e-6,
            "flat curve left the chord at {point:?}"
        );
    }
}

#[test]
fn curved_head_aims_along_the_end_tangent_not_the_chord() {
    // A head aimed down the chord on a strongly bent arrow points visibly
    // wrong. Break it by building the head from `axis.base()` and this fails.
    let skeleton = styled_skeleton(ArrowStyle::Curved, 0.8);
    let head = skeleton.head;
    let base = (
        (head.left.0 + head.right.0) / 2.0,
        (head.left.1 + head.right.1) / 2.0,
    );
    // Direction from the base to the tip: the axis the head is aimed along.
    let aim = (head.tip.0 - base.0, head.tip.1 - base.1);
    let aim_len = (aim.0 * aim.0 + aim.1 * aim.1).sqrt();
    let aim = (aim.0 / aim_len, aim.1 / aim_len);

    // The chord runs from the tail (0, 100) to the tip (400, 100): dead right.
    let chord = (1.0, 0.0);
    let cosine = aim.0 * chord.0 + aim.1 * chord.1;
    assert!(
        cosine < 0.95,
        "head is still aimed along the chord (cos = {cosine}); a bent arrow would point wrong"
    );

    // And it does aim along the curve: the last spine segment's direction.
    let points = skeleton.spine.points();
    let last = points[points.len() - 1];
    let previous = points[points.len() - 2];
    let tangent = (last.0 - previous.0, last.1 - previous.1);
    let tangent_len = (tangent.0 * tangent.0 + tangent.1 * tangent.1).sqrt();
    let tangent = (tangent.0 / tangent_len, tangent.1 / tangent_len);
    let alignment = aim.0 * tangent.0 + aim.1 * tangent.1;
    assert!(
        alignment > 0.99,
        "head is not aimed along the curve's end tangent (cos = {alignment})"
    );
}

#[test]
fn curve_sampling_scales_with_chord_length_within_bounds() {
    let short = calculate_arrow_skeleton(40, 0, 0, 0, 2.0, 10.0, 30.0, ArrowStyle::Curved, 0.3)
        .expect("short arrow should yield a skeleton");
    let long = calculate_arrow_skeleton(2000, 0, 0, 0, 2.0, 10.0, 30.0, ArrowStyle::Curved, 0.3)
        .expect("long arrow should yield a skeleton");

    // Floor of 12 segments (13 points) and ceiling of 64 (65 points).
    assert_eq!(short.spine.points().len(), 13);
    assert_eq!(long.spine.points().len(), 65);
}

#[test]
fn bend_is_clamped_to_the_supported_range() {
    assert_eq!(super::arrow::clamp_arrow_bend(5.0), 1.0);
    assert_eq!(super::arrow::clamp_arrow_bend(-5.0), -1.0);
    assert_eq!(super::arrow::clamp_arrow_bend(f64::NAN), 0.0);
    // An unclamped bend would let a session file draw an arc off-screen.
    let wild = styled_skeleton(ArrowStyle::Curved, 50.0);
    let capped = styled_skeleton(ArrowStyle::Curved, 1.0);
    assert_eq!(wild.spine.points(), capped.spine.points());
}

#[test]
fn a_uniform_scale_leaves_the_bend_alone() {
    // `bend` is a fraction of the chord, so chord and bulge grow together and
    // the stored number is already correct. Recomputing it anyway has to be
    // exactly the identity, or every resize would nudge the arc.
    let bend = scaled_arrow_bend(
        (0.0, 0.0),
        (100.0, 0.0),
        (0.0, 0.0),
        (200.0, 0.0),
        0.4,
        2.0,
        2.0,
    );
    assert!(
        (bend - 0.4).abs() < 1e-12,
        "uniform scale changed the bend to {bend}"
    );
}

#[test]
fn stretching_across_the_chord_grows_the_bend() {
    // The bug this exists for: dragging the bottom handle of a horizontal
    // curved arrow leaves the chord untouched, so a bend copied through
    // unchanged keeps the same bulge and the arc — the only part of that arrow
    // with any height — ignores the drag.
    let bend = scaled_arrow_bend(
        (0.0, 0.0),
        (100.0, 0.0),
        (0.0, 0.0),
        (100.0, 0.0),
        0.2,
        1.0,
        3.0,
    );
    assert!(
        (bend - 0.6).abs() < 1e-12,
        "tripling the height should triple the bend, got {bend}"
    );
}

#[test]
fn stretching_along_the_chord_shrinks_the_bend() {
    // The mirror image: widening an arrow without making it taller has to leave
    // the arc's height where it is, which as a *fraction* of a chord three
    // times longer is a third of the bend.
    let bend = scaled_arrow_bend(
        (0.0, 0.0),
        (100.0, 0.0),
        (0.0, 0.0),
        (300.0, 0.0),
        0.6,
        3.0,
        1.0,
    );
    assert!(
        (bend - 0.2).abs() < 1e-12,
        "widening should shrink the bend fraction, got {bend}"
    );
}

#[test]
fn naming_the_endpoints_backwards_gives_the_same_bend() {
    // The caller holds `(x1, y1)` and `(x2, y2)` and has no idea which is the
    // tail; `head_at_end` decides that. This is what lets it stay ignorant:
    // swapping the two flips the normal on both sides of the projection, so
    // the sign cancels rather than mirroring the arc.
    let forward = scaled_arrow_bend(
        (0.0, 0.0),
        (100.0, 0.0),
        (0.0, 0.0),
        (300.0, 0.0),
        0.6,
        3.0,
        1.0,
    );
    let backward = scaled_arrow_bend(
        (100.0, 0.0),
        (0.0, 0.0),
        (300.0, 0.0),
        (0.0, 0.0),
        0.6,
        3.0,
        1.0,
    );
    assert_eq!(forward, backward);
}

#[test]
fn a_scaled_bend_stays_in_range_and_survives_degenerate_input() {
    // Stretching hard enough to push the arc past a quarter-circle clamps
    // rather than producing geometry the sampler is not defined over.
    let extreme = scaled_arrow_bend(
        (0.0, 0.0),
        (100.0, 0.0),
        (0.0, 0.0),
        (100.0, 0.0),
        0.8,
        1.0,
        8.0,
    );
    assert_eq!(extreme, 1.0);

    // A chord collapsed to nothing has no normal to project onto, so the stored
    // bend is kept as-is instead of being replaced by a division by zero.
    let collapsed = scaled_arrow_bend(
        (0.0, 0.0),
        (100.0, 0.0),
        (5.0, 5.0),
        (5.0, 5.0),
        0.4,
        0.0,
        0.0,
    );
    assert_eq!(collapsed, 0.4);

    // A straight arrow has no arc to carry, and no scale can give it one.
    let straight = scaled_arrow_bend(
        (0.0, 0.0),
        (100.0, 0.0),
        (0.0, 0.0),
        (100.0, 0.0),
        0.0,
        1.0,
        5.0,
    );
    assert_eq!(straight, 0.0);
}
