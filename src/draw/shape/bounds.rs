use crate::util::{self, Rect};

use super::arrow_label::arrow_label_layout;
use super::types::ArrowLabel;

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

    let padding = stroke_padding(thick);
    min_x -= padding;
    max_x += padding;
    min_y -= padding;
    max_y += padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_line(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    thick: f64,
) -> Option<Rect> {
    let padding = stroke_padding(thick);

    let min_x = x1.min(x2) - padding;
    let max_x = x1.max(x2) + padding;
    let min_y = y1.min(y2) - padding;
    let max_y = y1.max(y2) + padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_rect(x: i32, y: i32, w: i32, h: i32, thick: f64) -> Option<Rect> {
    let padding = stroke_padding(thick);

    let x2 = x + w;
    let y2 = y + h;

    let min_x = x.min(x2) - padding;
    let max_x = x.max(x2) + padding;
    let min_y = y.min(y2) - padding;
    let max_y = y.max(y2) + padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

pub(crate) fn bounding_box_for_ellipse(
    cx: i32,
    cy: i32,
    rx: i32,
    ry: i32,
    thick: f64,
) -> Option<Rect> {
    let padding = stroke_padding(thick);
    let min_x = (cx - rx) - padding;
    let max_x = (cx + rx) + padding;
    let min_y = (cy - ry) - padding;
    let max_y = (cy + ry) + padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

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
    label: Option<&ArrowLabel>,
) -> Option<Rect> {
    let (tip_x, tip_y, tail_x, tail_y) = if head_at_end {
        (x2, y2, x1, y1)
    } else {
        (x1, y1, x2, y2)
    };

    let mut min_x = tip_x.min(tail_x) as f64;
    let mut max_x = tip_x.max(tail_x) as f64;
    let mut min_y = tip_y.min(tail_y) as f64;
    let mut max_y = tip_y.max(tail_y) as f64;

    if let Some(geometry) = util::calculate_arrowhead_triangle_custom(
        tip_x,
        tip_y,
        tail_x,
        tail_y,
        thick,
        arrow_length,
        arrow_angle,
    ) {
        min_x = min_x.min(geometry.left.0).min(geometry.right.0);
        max_x = max_x.max(geometry.left.0).max(geometry.right.0);
        min_y = min_y.min(geometry.left.1).min(geometry.right.1);
        max_y = max_y.max(geometry.left.1).max(geometry.right.1);
    }

    let padding = stroke_padding(thick) as f64;

    if let Some(label) = label {
        let label_text = label.value.to_string();
        if let Some(layout) = arrow_label_layout(
            tip_x,
            tip_y,
            tail_x,
            tail_y,
            thick,
            &label_text,
            label.size,
            &label.font_descriptor,
        ) {
            min_x = min_x.min(layout.bounds.x as f64);
            min_y = min_y.min(layout.bounds.y as f64);
            max_x = max_x.max((layout.bounds.x + layout.bounds.width) as f64);
            max_y = max_y.max((layout.bounds.y + layout.bounds.height) as f64);
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

    min_x -= padding;
    max_x += padding;
    min_y -= padding;
    max_y += padding;

    ensure_positive_rect(min_x, min_y, max_x, max_y)
}

fn stroke_padding(thick: f64) -> i32 {
    let padding = (thick / 2.0).ceil() as i32;
    padding.max(1)
}

fn ensure_positive_rect(min_x: i32, min_y: i32, max_x: i32, max_y: i32) -> Option<Rect> {
    let (min_x, max_x) = if min_x == max_x {
        (min_x, max_x + 1)
    } else {
        (min_x, max_x)
    };
    let (min_y, max_y) = if min_y == max_y {
        (min_y, max_y + 1)
    } else {
        (min_y, max_y)
    };
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

pub(crate) fn ensure_positive_rect_f64(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> Option<Rect> {
    let min_x = min_x.floor() as i32;
    let min_y = min_y.floor() as i32;
    let max_x = max_x.ceil() as i32;
    let max_y = max_y.ceil() as i32;
    ensure_positive_rect(min_x, min_y, max_x, max_y)
}
