use crate::util::{self, Rect};

use super::arrow_label::{arrow_label_ends, arrow_label_layout};
use super::types::{ArrowLabel, ArrowStyle};

const MIN_COORDINATE: i64 = i32::MIN as i64;
const MAX_COORDINATE_EXCLUSIVE: i64 = i32::MAX as i64 + 1;

pub(crate) fn bounding_box_for_points(points: &[(i32, i32)], thick: f64) -> Option<Rect> {
    if points.is_empty() {
        return None;
    }
    let mut min_x = points[0].0;
    let mut max_x = points[0].0;
    let mut min_y = points[0].1;
    let mut max_y = points[0].1;

    for &(x, y) in &points[1..] {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    padded_extrema_rect(min_x, min_y, max_x, max_y, stroke_padding(thick))
}

pub(super) fn bounding_box_for_pressure_points(points: &[(i32, i32, f32)]) -> Option<Rect> {
    let &(first_x, first_y, _) = points.first()?;
    let mut min_x = first_x;
    let mut max_x = first_x;
    let mut min_y = first_y;
    let mut max_y = first_y;
    let mut max_thick = 0.0f32;

    for &(x, y, thickness) in points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
        max_thick = max_thick.max(thickness);
    }

    let padding = i64::from((max_thick as i32 / 2).max(1));
    padded_extrema_rect(min_x, min_y, max_x, max_y, padding)
}

pub(crate) fn bounding_box_for_line(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    thick: f64,
) -> Option<Rect> {
    padded_extrema_rect(
        x1.min(x2),
        y1.min(y2),
        x1.max(x2),
        y1.max(y2),
        stroke_padding(thick),
    )
}

pub(crate) fn bounding_box_for_rect(x: i32, y: i32, w: i32, h: i32, thick: f64) -> Option<Rect> {
    let padding = stroke_padding(thick);
    let x = i64::from(x);
    let y = i64::from(y);
    let x2 = x + i64::from(w);
    let y2 = y + i64::from(h);

    ensure_positive_rect_i64(
        x.min(x2) - padding,
        y.min(y2) - padding,
        x.max(x2) + padding,
        y.max(y2) + padding,
    )
}

pub(crate) fn bounding_box_for_blur(x: i32, y: i32, w: i32, h: i32) -> Option<Rect> {
    let x = i64::from(x);
    let y = i64::from(y);
    let x2 = x + i64::from(w);
    let y2 = y + i64::from(h);
    let padding = 1_i64;
    ensure_positive_rect_i64(
        x.min(x2) - padding,
        y.min(y2) - padding,
        x.max(x2) + padding,
        y.max(y2) + padding,
    )
}

pub(crate) fn bounding_box_for_ellipse(
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    thick: f64,
) -> Option<Rect> {
    if rx < 0 || ry < 0 {
        return None;
    }

    let padding = stroke_padding(thick);
    let cx = i64::from(cx);
    let cy = i64::from(cy);
    let rx = i64::from(rx);
    let ry = i64::from(ry);

    ensure_positive_rect_i64(
        cx - rx - padding,
        cy - ry - padding,
        cx + rx + padding,
        cy + ry + padding,
    )
}

/// Dirty-region bounds for one arrow.
///
/// The endpoints alone are not enough once a style bends: a curved arrow's arc
/// bulges outside the chord's box, and an under-sized box leaves repaint trails
/// behind the shaft. The arc is unioned from the same sampler the renderer
/// walks, so the box cannot be tighter than what was drawn.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bounding_box_for_arrow(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    thick: f64,
    arrow_length: f64,
    arrow_angle: f64,
    head_at_end: bool,
    style: ArrowStyle,
    bend: f64,
    label: Option<&ArrowLabel>,
) -> Option<Rect> {
    let (tip_x, tip_y, tail_x, tail_y) = if head_at_end {
        (x2, y2, x1, y1)
    } else {
        (x1, y1, x2, y2)
    };
    // The label does not always share the head's reading of the endpoints:
    // `Double` has a head at each end, so `head_at_end` must not decide which
    // side of the shaft the number sits on.
    let (label_tip_x, label_tip_y, label_tail_x, label_tail_y) =
        arrow_label_ends(x1, y1, x2, y2, head_at_end, style);

    let mut min_x = tip_x.min(tail_x) as f64;
    let mut max_x = tip_x.max(tail_x) as f64;
    let mut min_y = tip_y.min(tail_y) as f64;
    let mut max_y = tip_y.max(tail_y) as f64;

    if let Some(skeleton) = util::calculate_arrow_skeleton(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
        style,
        bend,
    ) {
        let heads = [Some(skeleton.head), skeleton.tail_head];
        for corner in heads
            .iter()
            .flatten()
            .flat_map(|head| [head.tip, head.left, head.right])
            .chain(skeleton.spine.points().iter().copied())
        {
            min_x = min_x.min(corner.0);
            max_x = max_x.max(corner.0);
            min_y = min_y.min(corner.1);
            max_y = max_y.max(corner.1);
        }
    }

    let padding = stroke_padding(thick) as f64;

    if let Some(label) = label {
        let label_text = label.value.to_string();
        if let Some(layout) = arrow_label_layout(
            label_tip_x,
            label_tip_y,
            label_tail_x,
            label_tail_y,
            thick,
            style.effective_bend(bend),
            &label_text,
            label.size,
            &label.font_descriptor,
        ) {
            min_x = min_x.min(layout.bounds.x as f64);
            min_y = min_y.min(layout.bounds.y as f64);
            max_x = max_x.max((i64::from(layout.bounds.x) + i64::from(layout.bounds.width)) as f64);
            max_y =
                max_y.max((i64::from(layout.bounds.y) + i64::from(layout.bounds.height)) as f64);
        }
    }

    ensure_positive_rect_f64(
        min_x - padding,
        min_y - padding,
        max_x + padding,
        max_y + padding,
    )
}

pub(crate) fn bounding_box_for_eraser(points: &[(i32, i32)], diameter: f64) -> Option<Rect> {
    if points.is_empty() {
        return None;
    }
    let padding = stroke_padding(diameter.max(1.0));
    let mut min_x = points[0].0;
    let mut max_x = points[0].0;
    let mut min_y = points[0].1;
    let mut max_y = points[0].1;

    for &(x, y) in &points[1..] {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    padded_extrema_rect(min_x, min_y, max_x, max_y, padding)
}

fn stroke_padding(thick: f64) -> i64 {
    ((thick / 2.0).ceil() as i64).clamp(1, i64::from(i32::MAX))
}

#[cfg(test)]
fn ensure_positive_rect(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Option<Rect> {
    ensure_positive_rect_i64(
        i64::from(min_x),
        i64::from(min_y),
        i64::from(max_x),
        i64::from(max_y),
    )
}

fn padded_extrema_rect(
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    padding: i64,
) -> Option<Rect> {
    ensure_positive_rect_i64(
        i64::from(min_x) - padding,
        i64::from(min_y) - padding,
        i64::from(max_x) + padding,
        i64::from(max_y) + padding,
    )
}

pub(super) fn ensure_positive_rect_i64(
    min_x: i64,
    min_y: i64,
    max_x: i64,
    max_y: i64,
) -> Option<Rect> {
    if min_x > max_x || min_y > max_y {
        return None;
    }

    let max_x = if min_x == max_x {
        max_x.checked_add(1)?
    } else {
        max_x
    };
    let max_y = if min_y == max_y {
        max_y.checked_add(1)?
    } else {
        max_y
    };

    let min_x = min_x.clamp(MIN_COORDINATE, MAX_COORDINATE_EXCLUSIVE);
    let min_y = min_y.clamp(MIN_COORDINATE, MAX_COORDINATE_EXCLUSIVE);
    let max_x = max_x.clamp(MIN_COORDINATE, MAX_COORDINATE_EXCLUSIVE);
    let max_y = max_y.clamp(MIN_COORDINATE, MAX_COORDINATE_EXCLUSIVE);
    if min_x >= max_x || min_y >= max_y {
        return None;
    }

    Rect::new(
        i32::try_from(min_x).ok()?,
        i32::try_from(min_y).ok()?,
        i32::try_from(max_x - min_x).ok()?,
        i32::try_from(max_y - min_y).ok()?,
    )
}

pub(crate) fn ensure_positive_rect_f64(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> Option<Rect> {
    if ![min_x, min_y, max_x, max_y].into_iter().all(f64::is_finite) {
        return None;
    }

    ensure_positive_rect_i64(
        min_x.floor() as i64,
        min_y.floor() as i64,
        max_x.ceil() as i64,
        max_y.ceil() as i64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounding_box_for_points_returns_none_for_empty_input() {
        assert_eq!(bounding_box_for_points(&[], 2.0), None);
    }

    #[test]
    fn bounding_box_for_rect_handles_negative_drag_dimensions() {
        assert_eq!(
            bounding_box_for_rect(10, 20, -4, -6, 1.0),
            Rect::new(5, 13, 6, 8)
        );
    }

    #[test]
    fn ensure_positive_rect_makes_degenerate_integer_bounds_visible() {
        assert_eq!(ensure_positive_rect(5, 7, 5, 7), Rect::new(5, 7, 1, 1));
    }

    #[test]
    fn ensure_positive_rect_f64_uses_floor_and_ceil_bounds() {
        assert_eq!(
            ensure_positive_rect_f64(1.2, 3.8, 4.1, 6.0),
            Rect::new(1, 3, 4, 3)
        );
    }

    #[test]
    fn stroke_padding_never_drops_below_one_pixel() {
        assert_eq!(stroke_padding(0.0), 1);
        assert_eq!(stroke_padding(1.1), 1);
        assert_eq!(stroke_padding(2.1), 2);
    }

    #[test]
    fn bounding_box_for_blur_includes_outline_stroke() {
        assert_eq!(
            bounding_box_for_blur(10, 20, 30, 40),
            Rect::new(9, 19, 32, 42)
        );
        assert_eq!(
            bounding_box_for_blur(10, 20, -4, -6),
            Rect::new(5, 13, 6, 8)
        );
    }

    #[test]
    fn point_bounds_clip_padding_at_coordinate_edges() {
        let at_min = bounding_box_for_points(&[(i32::MIN, i32::MIN)], 2.0)
            .expect("minimum coordinate should retain in-domain bounds");
        assert!(at_min.contains(i32::MIN, i32::MIN));

        let at_max = bounding_box_for_points(&[(i32::MAX, i32::MAX)], 2.0)
            .expect("maximum coordinate should retain in-domain bounds");
        assert!(at_max.contains(i32::MAX, i32::MAX));
    }

    #[test]
    fn unrepresentable_full_span_bounds_fail_closed() {
        assert_eq!(bounding_box_for_line(i32::MIN, 0, i32::MAX, 0, 1.0), None);
        assert_eq!(
            bounding_box_for_points(&[(i32::MIN, 0), (i32::MAX, 0)], 1.0),
            None
        );
    }

    #[test]
    fn rectangle_like_bounds_use_checked_endpoint_arithmetic() {
        let rect = bounding_box_for_rect(i32::MAX, i32::MAX, i32::MAX, i32::MAX, 1.0)
            .expect("the visible clipped edge should remain representable");
        assert!(rect.contains(i32::MAX, i32::MAX));

        let blur = bounding_box_for_blur(i32::MAX, i32::MAX, i32::MAX, i32::MAX)
            .expect("the visible clipped blur edge should remain representable");
        assert!(blur.contains(i32::MAX, i32::MAX));
    }

    #[test]
    fn ellipse_bounds_validate_radii_and_clip_coordinate_edges() {
        assert_eq!(bounding_box_for_ellipse(0, 0, -1, 1, 1.0), None);

        let bounds = bounding_box_for_ellipse(i32::MAX, i32::MAX, 1, 1, 1.0)
            .expect("the visible clipped ellipse edge should remain representable");
        assert!(bounds.contains(i32::MAX, i32::MAX));
    }

    #[test]
    fn floating_bounds_reject_non_finite_values() {
        assert_eq!(ensure_positive_rect_f64(f64::NAN, 0.0, 1.0, 1.0), None);
        assert_eq!(ensure_positive_rect_f64(0.0, 0.0, f64::INFINITY, 1.0), None);
    }

    #[test]
    fn degenerate_bounds_at_maximum_coordinate_remain_visible() {
        assert_eq!(
            ensure_positive_rect_i64(
                i64::from(i32::MAX),
                i64::from(i32::MAX),
                i64::from(i32::MAX),
                i64::from(i32::MAX),
            ),
            Rect::new(i32::MAX, i32::MAX, 1, 1)
        );
    }
}
