//! Pure geometry transforms owned by `Shape`.

use super::Shape;

impl Shape {
    pub(crate) fn translate(&mut self, dx: i32, dy: i32) {
        match self {
            Shape::Freehand { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
            Shape::FreehandPressure { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
            Shape::Line { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Shape::Rect { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::Ellipse { cx, cy, .. } => {
                *cx += dx;
                *cy += dy;
            }
            Shape::Spotlight { cx, cy, .. } => {
                *cx += dx;
                *cy += dy;
            }
            Shape::Polygon { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
            Shape::Arrow { x1, y1, x2, y2, .. } => {
                *x1 += dx;
                *x2 += dx;
                *y1 += dy;
                *y2 += dy;
            }
            Shape::BlurRect { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::Image { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::Text { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::StepMarker { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::StickyNote { x, y, .. } => {
                *x += dx;
                *y += dy;
            }
            Shape::MarkerStroke { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
            Shape::EraserStroke { points, .. } => {
                for point in points {
                    point.0 += dx;
                    point.1 += dy;
                }
            }
        }
    }

    pub(crate) fn scaled(&self, scale_x: f64, scale_y: f64, anchor_x: f64, anchor_y: f64) -> Shape {
        match self {
            Shape::Rect {
                x,
                y,
                w,
                h,
                fill,
                color,
                thick,
            } => {
                let (nx, ny) = scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                let nw = scale_size(*w, scale_x);
                let nh = scale_size(*h, scale_y);
                Shape::Rect {
                    x: nx,
                    y: ny,
                    w: nw.max(1),
                    h: nh.max(1),
                    fill: *fill,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::Ellipse {
                cx,
                cy,
                rx,
                ry,
                fill,
                color,
                thick,
            } => {
                let (ncx, ncy) = scale_point_i32(*cx, *cy, anchor_x, anchor_y, scale_x, scale_y);
                let nrx = scale_size(*rx, scale_x);
                let nry = scale_size(*ry, scale_y);
                Shape::Ellipse {
                    cx: ncx,
                    cy: ncy,
                    rx: nrx.max(1),
                    ry: nry.max(1),
                    fill: *fill,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::Spotlight {
                cx,
                cy,
                rx,
                ry,
                magnification,
            } => {
                let (ncx, ncy) = scale_point_i32(*cx, *cy, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Spotlight {
                    cx: ncx,
                    cy: ncy,
                    rx: scale_size(*rx, scale_x).max(1),
                    ry: scale_size(*ry, scale_y).max(1),
                    magnification: *magnification,
                }
            }
            Shape::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                thick,
            } => {
                let (nx1, ny1) = scale_point_i32(*x1, *y1, anchor_x, anchor_y, scale_x, scale_y);
                let (nx2, ny2) = scale_point_i32(*x2, *y2, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Line {
                    x1: nx1,
                    y1: ny1,
                    x2: nx2,
                    y2: ny2,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::Arrow {
                x1,
                y1,
                x2,
                y2,
                color,
                thick,
                arrow_length,
                arrow_angle,
                head_at_end,
                style,
                bend,
                label,
            } => {
                let (nx1, ny1) = scale_point_i32(*x1, *y1, anchor_x, anchor_y, scale_x, scale_y);
                let (nx2, ny2) = scale_point_i32(*x2, *y2, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Arrow {
                    x1: nx1,
                    y1: ny1,
                    x2: nx2,
                    y2: ny2,
                    color: *color,
                    thick: *thick,
                    arrow_length: *arrow_length,
                    arrow_angle: *arrow_angle,
                    head_at_end: *head_at_end,
                    style: *style,
                    // `bend` is a fraction of the chord, so a uniform scale
                    // carries the arc for free — but a non-uniform one has to
                    // scale the arc itself, or the bulge (the only part of a
                    // flat curved arrow with any height) ignores the drag.
                    bend: crate::util::scaled_arrow_bend(
                        (*x1 as f64, *y1 as f64),
                        (*x2 as f64, *y2 as f64),
                        (nx1 as f64, ny1 as f64),
                        (nx2 as f64, ny2 as f64),
                        *bend,
                        scale_x,
                        scale_y,
                    ),
                    label: label.clone(),
                }
            }
            Shape::Polygon {
                kind,
                points,
                fill,
                color,
                thick,
            } => {
                let scaled_points = scale_points(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Polygon {
                    kind: *kind,
                    points: scaled_points,
                    fill: *fill,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::BlurRect {
                x,
                y,
                w,
                h,
                strength,
                style,
            } => {
                let (nx, ny) = scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                let nw = scale_size(*w, scale_x);
                let nh = scale_size(*h, scale_y);
                Shape::BlurRect {
                    x: nx,
                    y: ny,
                    w: nw.max(1),
                    h: nh.max(1),
                    strength: *strength,
                    style: *style,
                }
            }
            Shape::Image { x, y, w, h, data } => {
                let (nx, ny) = scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                let nw = scale_size(*w, scale_x);
                let nh = scale_size(*h, scale_y);
                Shape::Image {
                    x: nx,
                    y: ny,
                    w: nw.max(1),
                    h: nh.max(1),
                    data: data.clone(),
                }
            }
            Shape::Freehand {
                points,
                color,
                thick,
            } => {
                let scaled_points = scale_points(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Freehand {
                    points: scaled_points,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::FreehandPressure { points, color } => {
                let scaled_points =
                    scale_points_with_pressure(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::FreehandPressure {
                    points: scaled_points,
                    color: *color,
                }
            }
            Shape::MarkerStroke {
                points,
                color,
                thick,
            } => {
                let scaled_points = scale_points(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::MarkerStroke {
                    points: scaled_points,
                    color: *color,
                    thick: *thick,
                }
            }
            Shape::StepMarker { x, y, color, label } => {
                let (nx, ny) = scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                Shape::StepMarker {
                    x: nx,
                    y: ny,
                    color: *color,
                    label: label.clone(),
                }
            }
            Shape::EraserStroke { points, brush } => {
                let scaled_points = scale_points(points, anchor_x, anchor_y, scale_x, scale_y);
                Shape::EraserStroke {
                    points: scaled_points,
                    brush: brush.clone(),
                }
            }
            // Text and StickyNote: just move position, don't scale content
            Shape::Text {
                x,
                y,
                text,
                color,
                size,
                font_descriptor,
                background_enabled,
                wrap_width,
            } => {
                let (nx, ny) = scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                Shape::Text {
                    x: nx,
                    y: ny,
                    text: text.clone(),
                    color: *color,
                    size: *size,
                    font_descriptor: font_descriptor.clone(),
                    background_enabled: *background_enabled,
                    wrap_width: *wrap_width,
                }
            }
            Shape::StickyNote {
                x,
                y,
                text,
                background,
                size,
                font_descriptor,
                wrap_width,
            } => {
                let (nx, ny) = scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
                Shape::StickyNote {
                    x: nx,
                    y: ny,
                    text: text.clone(),
                    background: *background,
                    size: *size,
                    font_descriptor: font_descriptor.clone(),
                    wrap_width: *wrap_width,
                }
            }
        }
    }
}

fn scale_point(
    x: f64,
    y: f64,
    anchor_x: f64,
    anchor_y: f64,
    scale_x: f64,
    scale_y: f64,
) -> (f64, f64) {
    let dx = x - anchor_x;
    let dy = y - anchor_y;
    (anchor_x + dx * scale_x, anchor_y + dy * scale_y)
}

fn scale_point_i32(
    x: i32,
    y: i32,
    anchor_x: f64,
    anchor_y: f64,
    scale_x: f64,
    scale_y: f64,
) -> (i32, i32) {
    let (sx, sy) = scale_point(x as f64, y as f64, anchor_x, anchor_y, scale_x, scale_y);
    (sx.round() as i32, sy.round() as i32)
}

fn scale_size(size: i32, factor: f64) -> i32 {
    (size as f64 * factor).round() as i32
}

fn scale_points(
    points: &[(i32, i32)],
    anchor_x: f64,
    anchor_y: f64,
    scale_x: f64,
    scale_y: f64,
) -> Vec<(i32, i32)> {
    points
        .iter()
        .map(|(x, y)| scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y))
        .collect()
}

fn scale_points_with_pressure(
    points: &[(i32, i32, f32)],
    anchor_x: f64,
    anchor_y: f64,
    scale_x: f64,
    scale_y: f64,
) -> Vec<(i32, i32, f32)> {
    points
        .iter()
        .map(|(x, y, pressure)| {
            let (x, y) = scale_point_i32(*x, *y, anchor_x, anchor_y, scale_x, scale_y);
            (x, y, *pressure)
        })
        .collect()
}

#[cfg(test)]
mod tests;
