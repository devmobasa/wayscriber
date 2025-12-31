use cairo::Context;
use std::f64::consts::PI;

/// Draw an undo icon (curved arrow left)
pub fn draw_icon_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_curved_arrow(ctx, x, y, size, false);
}

/// Draw a redo icon (curved arrow right)
pub fn draw_icon_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    // Mirror the undo arrow for perfect symmetry
    ctx.save().ok();
    ctx.translate(x + size, y);
    ctx.scale(-1.0, 1.0);
    draw_curved_arrow(ctx, 0.0, 0.0, size, false);
    ctx.restore().ok();
}

/// Draw an undo all icon (double curved arrow left)
pub fn draw_icon_undo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_double_curved_arrow(ctx, x, y, size);
}

/// Draw a redo all icon (double curved arrow right)
pub fn draw_icon_redo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    ctx.save().ok();
    ctx.translate(x + size, y);
    ctx.scale(-1.0, 1.0);
    draw_double_curved_arrow(ctx, 0.0, 0.0, size);
    ctx.restore().ok();
}

/// Draw an undo all delay icon (double curved arrow left with clock)
pub fn draw_icon_undo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Double arrow
    draw_double_curved_arrow(ctx, x, y, s);

    // Small clock indicator at bottom-right, tucked into the corner
    let clock_r = s * 0.12;
    let clock_x = x + s * 0.83;
    let clock_y = y + s * 0.83;
    ctx.arc(clock_x, clock_y, clock_r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Clock hands
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x, clock_y - clock_r * 0.55);
    let _ = ctx.stroke();
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x + clock_r * 0.55, clock_y);
    let _ = ctx.stroke();
}

/// Draw a redo all delay icon (double curved arrow right with clock)
pub fn draw_icon_redo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Mirror the double arrow
    ctx.save().ok();
    ctx.translate(x + s, y);
    ctx.scale(-1.0, 1.0);
    draw_double_curved_arrow(ctx, 0.0, 0.0, s);
    ctx.restore().ok();

    // Small clock indicator at bottom-left
    let clock_r = s * 0.12;
    let clock_x = x + s * 0.17;
    let clock_y = y + s * 0.83;
    ctx.arc(clock_x, clock_y, clock_r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Clock hands
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x, clock_y - clock_r * 0.55);
    let _ = ctx.stroke();
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x + clock_r * 0.55, clock_y);
    let _ = ctx.stroke();
}

/// Draw a step undo icon (curved arrow with number)
pub fn draw_icon_step_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow (upper portion)
    ctx.arc_negative(x + s * 0.5, y + s * 0.35, s * 0.2, PI * 0.15, PI * 1.05);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.28, y + s * 0.42);
    ctx.line_to(x + s * 0.36, y + s * 0.28);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.28, y + s * 0.42);
    ctx.line_to(x + s * 0.44, y + s * 0.40);
    let _ = ctx.stroke();

    // Small "N" indicator at bottom center
    ctx.set_line_width((s * 0.06).max(1.0));
    let ny = y + s * 0.72;
    let nx = x + s * 0.5;
    // Draw "N" shape
    ctx.move_to(nx - s * 0.1, ny + s * 0.08);
    ctx.line_to(nx - s * 0.1, ny - s * 0.08);
    ctx.line_to(nx + s * 0.1, ny + s * 0.08);
    ctx.line_to(nx + s * 0.1, ny - s * 0.08);
    let _ = ctx.stroke();
}

/// Draw a step redo icon (curved arrow with number)
pub fn draw_icon_step_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow (upper portion)
    ctx.arc(x + s * 0.5, y + s * 0.35, s * 0.2, PI * 0.85, -PI * 0.05);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.72, y + s * 0.42);
    ctx.line_to(x + s * 0.64, y + s * 0.28);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.72, y + s * 0.42);
    ctx.line_to(x + s * 0.56, y + s * 0.40);
    let _ = ctx.stroke();

    // Small "N" indicator at bottom center
    ctx.set_line_width((s * 0.06).max(1.0));
    let ny = y + s * 0.72;
    let nx = x + s * 0.5;
    // Draw "N" shape
    ctx.move_to(nx - s * 0.1, ny + s * 0.08);
    ctx.line_to(nx - s * 0.1, ny - s * 0.08);
    ctx.line_to(nx + s * 0.1, ny + s * 0.08);
    ctx.line_to(nx + s * 0.1, ny - s * 0.08);
    let _ = ctx.stroke();
}

/// Common helper to draw a single curved arrow pointing left.
/// Set `offset_y` when you want a subtle vertical shift (not used here, but kept
/// for compatibility with older call sites).
fn draw_curved_arrow(ctx: &Context, x: f64, y: f64, size: f64, offset_y: bool) {
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
fn draw_double_curved_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
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
