use super::types::EraserReplayContext;
use crate::draw::Color;
use crate::draw::shape::{EraserBrush, EraserKind};

/// Render freehand stroke (polyline through points)
///
/// This function accepts a borrowed slice, avoiding clones for better performance.
/// Use this for rendering provisional shapes during drawing to prevent quadratic behavior.
pub fn render_freehand_borrowed(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    color: Color,
    thick: f64,
) {
    if points.is_empty() {
        return;
    }

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);
    ctx.set_line_width(thick);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Start at first point
    let (x0, y0) = points[0];
    ctx.move_to(x0 as f64, y0 as f64);

    // Draw lines through all points
    for &(x, y) in &points[1..] {
        ctx.line_to(x as f64, y as f64);
    }

    let _ = ctx.stroke();
}

/// Render a variable-thickness freehand stroke (pressure sensitive)
pub fn render_freehand_pressure_borrowed(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    thicknesses: &[f32],
    color: Color,
) {
    if points.is_empty() || thicknesses.is_empty() {
        return;
    }

    // Safety check: ensure lengths match or take minimum
    let len = points.len().min(thicknesses.len());
    if len < 2 {
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        for i in 0..len {
            let (x, y) = points[i];
            let t = thicknesses[i];
            ctx.new_path();
            ctx.arc(
                x as f64,
                y as f64,
                (t as f64) / 2.0,
                0.0,
                std::f64::consts::PI * 2.0,
            );
            ctx.fill().ok();
        }
        return;
    }

    ctx.set_source_rgba(color.r, color.g, color.b, color.a);

    // Helper to get smoothed point at index i
    // Applies a simple 3-point moving average
    let get_smoothed = |i: usize| -> (f64, f64, f64) {
        if i == 0 || i >= len - 1 {
            let (x, y) = points[i];
            (x as f64, y as f64, thicknesses[i] as f64)
        } else {
            let (x0, y0) = points[i - 1];
            let t0 = thicknesses[i - 1];

            let (x1, y1) = points[i];
            let t1 = thicknesses[i];

            let (x2, y2) = points[i + 1];
            let t2 = thicknesses[i + 1];

            (
                (x0 as f64 + x1 as f64 + x2 as f64) / 3.0,
                (y0 as f64 + y1 as f64 + y2 as f64) / 3.0,
                (t0 as f64 + t1 as f64 + t2 as f64) / 3.0,
            )
        }
    };

    let mut left_points = Vec::with_capacity(len);
    let mut right_points = Vec::with_capacity(len);

    for i in 0..len {
        let (cx, cy, ct) = get_smoothed(i);
        let p_curr = (cx, cy);

        // Determine the direction vector at this point using smoothed neighbors
        let dir = if len < 2 {
            (1.0, 0.0)
        } else if i == 0 {
            let (nx, ny, _) = get_smoothed(1);
            (nx - p_curr.0, ny - p_curr.1)
        } else if i == len - 1 {
            let (px, py, _) = get_smoothed(i - 1);
            (p_curr.0 - px, p_curr.1 - py)
        } else {
            let (px, py, _) = get_smoothed(i - 1);
            let (nx, ny, _) = get_smoothed(i + 1);
            (nx - px, ny - py)
        };

        let len_sq = dir.0 * dir.0 + dir.1 * dir.1;
        if len_sq < 0.000001 {
            if i > 0 && !left_points.is_empty() {
                let last_l = *left_points.last().unwrap();
                let last_r = *right_points.last().unwrap();
                left_points.push(last_l);
                right_points.push(last_r);
                continue;
            } else {
                continue;
            }
        }

        let inv_len = 1.0 / len_sq.sqrt();
        let unit_dir = (dir.0 * inv_len, dir.1 * inv_len);

        // Normal vector (-y, x)
        let normal = (-unit_dir.1, unit_dir.0);

        let half_thick = ct / 2.0;

        left_points.push((
            p_curr.0 + normal.0 * half_thick,
            p_curr.1 + normal.1 * half_thick,
        ));

        right_points.push((
            p_curr.0 - normal.0 * half_thick,
            p_curr.1 - normal.1 * half_thick,
        ));
    }

    // Draw the path
    if !left_points.is_empty() {
        ctx.new_path();

        // Go down the left side
        ctx.move_to(left_points[0].0, left_points[0].1);

        if left_points.len() > 2 {
            for i in 0..left_points.len() - 1 {
                let p0 = left_points[i];
                let p1 = left_points[i + 1];

                let mid_x = (p0.0 + p1.0) / 2.0;
                let mid_y = (p0.1 + p1.1) / 2.0;

                if i == 0 {
                    ctx.line_to(mid_x, mid_y);
                } else {
                    let (current_x, current_y) = ctx.current_point().unwrap_or((p0.0, p0.1));
                    let cp1_x = current_x + (2.0 / 3.0) * (p0.0 - current_x);
                    let cp1_y = current_y + (2.0 / 3.0) * (p0.1 - current_y);
                    let cp2_x = mid_x + (2.0 / 3.0) * (p0.0 - mid_x);
                    let cp2_y = mid_y + (2.0 / 3.0) * (p0.1 - mid_y);
                    ctx.curve_to(cp1_x, cp1_y, cp2_x, cp2_y, mid_x, mid_y);
                }
            }
            let last = *left_points.last().unwrap();
            ctx.line_to(last.0, last.1);
        } else {
            for p in left_points.iter().skip(1) {
                ctx.line_to(p.0, p.1);
            }
        }

        // Round cap at end
        let (ex, ey, et) = get_smoothed(left_points.len() - 1);
        let last_left = *left_points.last().unwrap();
        let end_angle = (last_left.1 - ey).atan2(last_left.0 - ex);
        ctx.arc_negative(
            ex,
            ey,
            et / 2.0,
            end_angle,
            end_angle - std::f64::consts::PI,
        );

        // Go up the right side
        if right_points.len() > 2 {
            let start_idx = right_points.len() - 1;
            let p_last = right_points[start_idx];
            let p_prev = right_points[start_idx - 1];
            let mid_x = (p_last.0 + p_prev.0) / 2.0;
            let mid_y = (p_last.1 + p_prev.1) / 2.0;

            ctx.line_to(mid_x, mid_y);

            for i in (1..start_idx).rev() {
                let p_curr = right_points[i];
                let p_next = right_points[i - 1];
                let mid_x = (p_curr.0 + p_next.0) / 2.0;
                let mid_y = (p_curr.1 + p_next.1) / 2.0;
                let (current_x, current_y) = ctx.current_point().unwrap_or((p_curr.0, p_curr.1));
                let cp1_x = current_x + (2.0 / 3.0) * (p_curr.0 - current_x);
                let cp1_y = current_y + (2.0 / 3.0) * (p_curr.1 - current_y);
                let cp2_x = mid_x + (2.0 / 3.0) * (p_curr.0 - mid_x);
                let cp2_y = mid_y + (2.0 / 3.0) * (p_curr.1 - mid_y);
                ctx.curve_to(cp1_x, cp1_y, cp2_x, cp2_y, mid_x, mid_y);
            }
            let first = right_points[0];
            ctx.line_to(first.0, first.1);
        } else {
            for p in right_points.iter().rev() {
                ctx.line_to(p.0, p.1);
            }
        }

        // Round cap at start
        let (sx, sy, st) = get_smoothed(0);
        let first_right = right_points[0];
        let start_angle = (first_right.1 - sy).atan2(first_right.0 - sx);
        ctx.arc_negative(
            sx,
            sy,
            st / 2.0,
            start_angle,
            start_angle - std::f64::consts::PI,
        );

        ctx.close_path();
        ctx.fill().ok();
    }
}

#[allow(dead_code)] // Used by the Wayland backend; the lib crate doesn't compile backend modules.
pub(crate) fn render_eraser_stroke(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    brush: &EraserBrush,
    eraser_ctx: &EraserReplayContext,
) {
    if points.is_empty() {
        return;
    }

    let stroke_width = brush.size.max(1.0);
    let (cap, join) = match brush.kind {
        EraserKind::Circle => (cairo::LineCap::Round, cairo::LineJoin::Round),
        EraserKind::Rect => (cairo::LineCap::Square, cairo::LineJoin::Miter),
    };

    let build_path = |ctx: &cairo::Context| {
        if points.len() == 1 {
            let (x, y) = (points[0].0 as f64, points[0].1 as f64);
            let half = stroke_width / 2.0;
            match brush.kind {
                EraserKind::Circle => ctx.arc(x, y, half, 0.0, std::f64::consts::PI * 2.0),
                EraserKind::Rect => ctx.rectangle(x - half, y - half, stroke_width, stroke_width),
            }
            return;
        }

        let (x0, y0) = points[0];
        ctx.move_to(x0 as f64, y0 as f64);
        for &(x, y) in &points[1..] {
            ctx.line_to(x as f64, y as f64);
        }
    };

    let _ = ctx.save();
    ctx.set_line_width(stroke_width);
    ctx.set_line_cap(cap);
    ctx.set_line_join(join);

    // Clear pass
    build_path(ctx);
    ctx.set_operator(cairo::Operator::Clear);
    let _ = ctx.stroke();

    // Paint background back into the cleared region, if available
    if let Some(pattern) = eraser_ctx.pattern {
        build_path(ctx);
        ctx.set_operator(cairo::Operator::Over);
        let _ = ctx.set_source(pattern);
        let _ = ctx.stroke();
    } else if let Some(color) = eraser_ctx.bg_color {
        build_path(ctx);
        ctx.set_operator(cairo::Operator::Over);
        ctx.set_source_rgba(color.r, color.g, color.b, color.a);
        let _ = ctx.stroke();
    }

    let _ = ctx.restore();
}

/// Render a marker stroke with soft edges and screen blending to mimic a physical highlighter.
pub fn render_marker_stroke_borrowed(
    ctx: &cairo::Context,
    points: &[(i32, i32)],
    color: Color,
    thick: f64,
) {
    if points.is_empty() {
        return;
    }

    // Reduce opacity to keep underlying text visible; clamp to avoid invisible strokes.
    let base_alpha = (color.a * 0.32).clamp(0.05, 0.85);
    let soft_width = (thick * 1.25).max(thick + 1.0);

    let draw_pass = |ctx: &cairo::Context, width: f64, alpha: f64| {
        ctx.set_source_rgba(color.r, color.g, color.b, alpha);
        ctx.set_line_width(width);
        ctx.set_line_cap(cairo::LineCap::Round);
        ctx.set_line_join(cairo::LineJoin::Round);
        let (x0, y0) = points[0];
        ctx.move_to(x0 as f64, y0 as f64);
        for &(x, y) in &points[1..] {
            ctx.line_to(x as f64, y as f64);
        }
        let _ = ctx.stroke();
    };

    ctx.save().ok();
    ctx.set_operator(cairo::Operator::Screen);
    // Soft outer pass for feathered edges
    draw_pass(ctx, soft_width, base_alpha * 0.7);
    // Core pass for body of the marker
    draw_pass(ctx, thick, base_alpha);
    ctx.restore().ok();
}
