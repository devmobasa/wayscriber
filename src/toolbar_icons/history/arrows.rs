use cairo::Context;
use std::f64::consts::PI;

/// Common helper to draw a single curved arrow pointing left.
/// Set `offset_y` when you want a subtle vertical shift (not used here, but kept
/// for compatibility with older call sites).
pub(super) fn draw_curved_arrow(ctx: &Context, x: f64, y: f64, size: f64, offset_y: bool) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5 + if offset_y { s * 0.02 } else { 0.0 };
    let r = s * 0.38;

    // Sweep around most of the circle, leaving a small gap on the left where the head sits.
    let start_angle = -PI * 0.9;
    let end_angle = PI * 0.9;

    ctx.save().ok();
    // Slight clockwise tilt to match the rest of the toolbar
    ctx.translate(cx, cy);
    ctx.rotate(-10f64.to_radians());
    ctx.translate(-cx, -cy);

    draw_arc_with_head(ctx, cx, cy, r, start_angle, end_angle, s * 0.18);

    ctx.restore().ok();
}

/// Helper to draw the "undo all / redo all" double curved arrow.
///
/// Front arrow = same geometry as the single undo/redo.
/// Back arrow   = smaller radius, shifted slightly, shorter arc so the heads
///                are clearly separated.
pub(super) fn draw_double_curved_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;

    // Base stroke for the front arrow
    let base_stroke = (s * 0.10).max(1.5);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Center of the icon
    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    // Same angles as the single curved arrow
    let start_angle = -PI * 0.9;
    let end_angle = PI * 0.9;

    // Front (main) arrow
    let r_front = s * 0.38;
    let head_front = s * 0.18;

    // Back arrow: slightly smaller radius, shifted down/right,
    // and with an earlier end angle so the heads don't overlap.
    let r_back = s * 0.30;
    let head_back = s * 0.15;
    let back_dx = s * 0.05;
    let back_dy = s * 0.03;

    // Angular separation between the two heads (~63 deg)
    let head_separation = PI * 0.35;
    let end_back = end_angle - head_separation;

    ctx.save().ok();
    // Same slight clockwise tilt as the single undo/redo
    ctx.translate(cx, cy);
    ctx.rotate(-10f64.to_radians());
    ctx.translate(-cx, -cy);

    // Draw back arrow first (thinner, so it visually sits behind)
    ctx.set_line_width(base_stroke * 0.85);
    draw_arc_with_head(
        ctx,
        cx + back_dx,
        cy + back_dy,
        r_back,
        start_angle,
        end_back,
        head_back,
    );

    // Draw front arrow on top
    ctx.set_line_width(base_stroke);
    draw_arc_with_head(ctx, cx, cy, r_front, start_angle, end_angle, head_front);

    ctx.restore().ok();
}

/// Draw an arc and a triangular arrow head whose direction follows the tangent
/// at the end of the arc. The head is constructed so both sides stay clear of
/// the circular stroke even at small icon sizes.
fn draw_arc_with_head(
    ctx: &Context,
    cx: f64,
    cy: f64,
    r: f64,
    start_angle: f64,
    end_angle: f64,
    head_len: f64,
) {
    // Circular portion.
    ctx.arc(cx, cy, r, start_angle, end_angle);
    let _ = ctx.stroke();

    // Arrow tip at the end of the arc.
    let tip_x = cx + r * end_angle.cos();
    let tip_y = cy + r * end_angle.sin();

    // Tangent direction (direction of travel along the arc).
    let dir = end_angle + PI / 2.0;

    // Build a proper isosceles triangle for the head:
    // - move back a bit along the tangent
    // - then offset to each side along the normal.
    let back_offset = head_len * 0.7;
    let side_len = head_len * 0.8;

    let base_cx = tip_x - back_offset * dir.cos();
    let base_cy = tip_y - back_offset * dir.sin();

    let normal = dir + PI / 2.0;
    let base1_x = base_cx + side_len * normal.cos();
    let base1_y = base_cy + side_len * normal.sin();
    let base2_x = base_cx - side_len * normal.cos();
    let base2_y = base_cy - side_len * normal.sin();

    // Outer edge of head
    ctx.move_to(tip_x, tip_y);
    ctx.line_to(base1_x, base1_y);
    let _ = ctx.stroke();

    // Inner edge of head
    ctx.move_to(tip_x, tip_y);
    ctx.line_to(base2_x, base2_y);
    let _ = ctx.stroke();
}
