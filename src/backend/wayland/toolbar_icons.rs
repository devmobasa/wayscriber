//! Icon drawing functions for the toolbar UI.
//!
//! All icons are drawn using Cairo paths for perfect scaling at any DPI.

use cairo::Context;
use std::f64::consts::PI;

/// Parameters for drawing curved arrows (undo/redo icons).
struct ArrowParams {
    stroke_width: f64,
    radius: f64,
    head_length: f64,
    head_spread: f64,
    start_angle: f64,
    end_angle: f64,
}

/// Draw a pen/freehand icon (pencil with wavy stroke)
pub fn draw_icon_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Pencil body - angled rectangle
    let px = x + s * 0.55;  // Pencil center x
    let py = y + s * 0.35;  // Pencil center y
    let angle = PI * 0.75; // 135 degrees (diagonal down-left)
    let body_len = s * 0.4;
    let body_w = s * 0.12;

    let _ = ctx.save();
    ctx.translate(px, py);
    ctx.rotate(angle);

    // Pencil body outline
    ctx.rectangle(-body_len / 2.0, -body_w / 2.0, body_len, body_w);
    let _ = ctx.stroke();

    // Pencil tip (triangle)
    ctx.move_to(-body_len / 2.0, -body_w / 2.0);
    ctx.line_to(-body_len / 2.0 - s * 0.12, 0.0);
    ctx.line_to(-body_len / 2.0, body_w / 2.0);
    let _ = ctx.stroke();

    // Eraser band at top
    ctx.move_to(body_len / 2.0 - s * 0.05, -body_w / 2.0);
    ctx.line_to(body_len / 2.0 - s * 0.05, body_w / 2.0);
    let _ = ctx.stroke();

    let _ = ctx.restore();

    // Wavy freehand stroke below pencil
    ctx.set_line_width((s * 0.1).max(1.5));
    ctx.move_to(x + s * 0.15, y + s * 0.72);
    ctx.curve_to(
        x + s * 0.3, y + s * 0.62,
        x + s * 0.45, y + s * 0.82,
        x + s * 0.65, y + s * 0.72,
    );
    let _ = ctx.stroke();
}

/// Draw a line tool icon
pub fn draw_icon_line(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x + s * 0.2, y + s * 0.8);
    ctx.line_to(x + s * 0.8, y + s * 0.2);
    let _ = ctx.stroke();
}

/// Draw a rectangle tool icon
pub fn draw_icon_rect(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);

    let margin = s * 0.2;
    ctx.rectangle(x + margin, y + margin, s - margin * 2.0, s - margin * 2.0);
    let _ = ctx.stroke();
}

/// Draw a circle/ellipse tool icon
pub fn draw_icon_circle(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;
    let r = s * 0.35;
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw an arrow tool icon
pub fn draw_icon_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Arrow line
    ctx.move_to(x + s * 0.2, y + s * 0.8);
    ctx.line_to(x + s * 0.8, y + s * 0.2);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.8, y + s * 0.2);
    ctx.line_to(x + s * 0.55, y + s * 0.25);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.8, y + s * 0.2);
    ctx.line_to(x + s * 0.75, y + s * 0.45);
    let _ = ctx.stroke();
}

/// Draw an eraser tool icon
#[allow(dead_code)]
pub fn draw_icon_eraser(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Eraser body (rotated rectangle)
    ctx.move_to(x + s * 0.3, y + s * 0.75);
    ctx.line_to(x + s * 0.15, y + s * 0.5);
    ctx.line_to(x + s * 0.5, y + s * 0.25);
    ctx.line_to(x + s * 0.85, y + s * 0.25);
    ctx.line_to(x + s * 0.85, y + s * 0.5);
    ctx.line_to(x + s * 0.5, y + s * 0.75);
    ctx.close_path();
    let _ = ctx.stroke();

    // Dividing line for eraser tip
    ctx.move_to(x + s * 0.5, y + s * 0.25);
    ctx.line_to(x + s * 0.5, y + s * 0.75);
    let _ = ctx.stroke();
}

/// Draw a text tool icon (letter T)
pub fn draw_icon_text(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(2.0);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Top bar of T
    ctx.move_to(x + s * 0.2, y + s * 0.25);
    ctx.line_to(x + s * 0.8, y + s * 0.25);
    let _ = ctx.stroke();

    // Vertical bar of T
    ctx.move_to(x + s * 0.5, y + s * 0.25);
    ctx.line_to(x + s * 0.5, y + s * 0.8);
    let _ = ctx.stroke();
}

/// Draw a highlighter tool icon (cursor with click ripple effect)
#[allow(dead_code)]
pub fn draw_icon_highlight(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Pointer/cursor arrow
    ctx.move_to(x + s * 0.2, y + s * 0.15);
    ctx.line_to(x + s * 0.2, y + s * 0.65);
    ctx.line_to(x + s * 0.35, y + s * 0.52);
    ctx.line_to(x + s * 0.5, y + s * 0.75);
    ctx.line_to(x + s * 0.58, y + s * 0.7);
    ctx.line_to(x + s * 0.43, y + s * 0.47);
    ctx.line_to(x + s * 0.58, y + s * 0.4);
    ctx.close_path();
    let _ = ctx.fill();

    // Ripple circles around click point
    ctx.set_line_width((s * 0.08).max(1.0));
    // Inner ripple
    ctx.arc(x + s * 0.7, y + s * 0.55, s * 0.12, 0.0, PI * 2.0);
    let _ = ctx.stroke();
    // Outer ripple
    ctx.arc(x + s * 0.7, y + s * 0.55, s * 0.22, 0.0, PI * 2.0);
    let _ = ctx.stroke();
}

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
    draw_double_curved_arrow(ctx, x, y, size, false);
}

/// Draw a redo all icon (double curved arrow right)
pub fn draw_icon_redo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    ctx.save().ok();
    ctx.translate(x + size, y);
    ctx.scale(-1.0, 1.0);
    draw_double_curved_arrow(ctx, 0.0, 0.0, size, false);
    ctx.restore().ok();
}

/// Draw an undo all delay icon (double curved arrow left with clock)
pub fn draw_icon_undo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Reuse the double-arrow shape and add a small clock
    draw_double_curved_arrow(ctx, x, y, s, false);

    // Improved clock badge: 8% smaller, positioned along arc tangent at bottom-right
    let clock_r = s * 0.129;  // Reduced from 0.14 (8% smaller)

    // Position badge 15% further from center, aligned to arc curve
    // Arc is at (0.5, 0.5) with radius ~0.38, rotated -10°
    // Place at bottom-right quadrant following the curve
    let badge_angle = PI * 0.35;  // ~63° - bottom-right along curve
    let badge_distance = s * 0.56;  // 15% further than arc edge
    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    let clock_x = cx + badge_distance * badge_angle.cos();
    let clock_y = cy + badge_distance * badge_angle.sin();

    ctx.arc(clock_x, clock_y, clock_r, 0.0, PI * 2.0);
    let _ = ctx.stroke();
    // Clock hands
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x, clock_y - clock_r * 0.6);
    let _ = ctx.stroke();
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x + clock_r * 0.5, clock_y);
    let _ = ctx.stroke();
}

/// Draw a redo all delay icon (double curved arrow right with clock)
pub fn draw_icon_redo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.save().ok();
    ctx.translate(x + s, y);
    ctx.scale(-1.0, 1.0);
    draw_double_curved_arrow(ctx, 0.0, 0.0, s, false);
    ctx.restore().ok();

    // Improved clock badge: 8% smaller, positioned along arc tangent at bottom-left
    let clock_r = s * 0.129;  // Reduced from 0.14 (8% smaller)

    // Position badge 15% further from center, aligned to arc curve
    // Mirror of undo delay - bottom-left quadrant
    let badge_angle = PI * 0.65;  // ~117° - bottom-left along curve
    let badge_distance = s * 0.56;  // 15% further than arc edge
    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    let clock_x = cx + badge_distance * badge_angle.cos();
    let clock_y = cy + badge_distance * badge_angle.sin();

    ctx.arc(clock_x, clock_y, clock_r, 0.0, PI * 2.0);
    let _ = ctx.stroke();
    // Clock hands
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x, clock_y - clock_r * 0.6);
    let _ = ctx.stroke();
    ctx.move_to(clock_x, clock_y);
    ctx.line_to(clock_x + clock_r * 0.5, clock_y);
    let _ = ctx.stroke();
}


/// Common helper to draw a single curved arrow pointing left.
/// Set `offset_y` when stacking multiple arcs.
fn draw_curved_arrow(ctx: &Context, x: f64, y: f64, size: f64, offset_y: bool) {
    let s = size;

    // Improved arrow parameters: 10% smaller head, tighter gap
    let params = ArrowParams {
        stroke_width: (s * 0.12).max(1.5),
        radius: s * 0.38,
        head_length: s * 0.162,  // Reduced from 0.18 (10% smaller)
        head_spread: PI / 7.0,   // ~25.7° spread
        start_angle: -PI * 0.9,
        end_angle: PI * 0.85,    // Tightened from 0.9 (5° gap increase)
    };

    ctx.set_line_width(params.stroke_width);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Centre of the circular part.
    let cx = x + s * 0.5;
    let cy = y + s * 0.5 + if offset_y { s * 0.02 } else { 0.0 };

    // Slight clockwise tilt to match the rest of the toolbar.
    ctx.save().ok();
    ctx.translate(cx, cy);
    ctx.rotate(-10f64.to_radians());
    ctx.translate(-cx, -cy);

    draw_arc_with_head(ctx, cx, cy, &params);

    ctx.restore().ok();
}

/// Helper to draw a double curved arrow (Undo All / Redo All).
fn draw_double_curved_arrow(ctx: &Context, x: f64, y: f64, size: f64, _offset: bool) {
    let s = size;

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    // Back arrow: slightly smaller and shifted down/right
    let params_back = ArrowParams {
        stroke_width: (s * 0.09).max(1.5),  // Reduced from 0.10
        radius: s * 0.38 * 0.72,
        head_length: s * 0.144,  // 0.16 * 0.9 (10% reduction)
        head_spread: PI / 7.0,
        start_angle: -PI * 0.9,
        end_angle: PI * 0.85,  // Tightened gap
    };

    // Front arrow: same geometry as normal undo
    let params_front = ArrowParams {
        stroke_width: (s * 0.09).max(1.5),  // Reduced from 0.10
        radius: s * 0.38,
        head_length: s * 0.18,   // Slightly larger than back for depth
        head_spread: PI / 7.0,
        start_angle: -PI * 0.9,
        end_angle: PI * 0.85,  // Tightened gap
    };

    let back_dx = s * 0.07;
    let back_dy = s * 0.07;

    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    ctx.save().ok();
    ctx.translate(cx, cy);
    ctx.rotate(-10f64.to_radians());
    ctx.translate(-cx, -cy);

    // Back arrow first (so it visually sits behind).
    ctx.set_line_width(params_back.stroke_width);
    draw_arc_with_head(ctx, cx + back_dx, cy + back_dy, &params_back);

    // Front arrow on top.
    ctx.set_line_width(params_front.stroke_width);
    draw_arc_with_head(ctx, cx, cy, &params_front);

    ctx.restore().ok();
}

/// Draw an arc with a head whose direction follows the arc tangent.
/// The head geometry is chosen so both sides are clearly visible and
/// don't collapse onto the circular stroke.
fn draw_arc_with_head(ctx: &Context, cx: f64, cy: f64, params: &ArrowParams) {
    // Circular part.
    ctx.arc(cx, cy, params.radius, params.start_angle, params.end_angle);
    let _ = ctx.stroke();

    // Tip of the arrow on the circle.
    let tip_x = cx + params.radius * params.end_angle.cos();
    let tip_y = cy + params.radius * params.end_angle.sin();

    // Direction of travel along the arc at the tip (base -> tip).
    // For cairo_arc, the path follows the direction of increasing angles.
    let dir = params.end_angle + PI / 2.0;

    // Compute two base points for the head, one slightly inside the circle
    // and one slightly outside, so the triangular shape reads clearly.
    let base1_x = tip_x - params.head_length * (dir + params.head_spread).cos();
    let base1_y = tip_y - params.head_length * (dir + params.head_spread).sin();
    let base2_x = tip_x - params.head_length * (dir - params.head_spread).cos();
    let base2_y = tip_y - params.head_length * (dir - params.head_spread).sin();

    // Outer side of the head.
    ctx.move_to(tip_x, tip_y);
    ctx.line_to(base1_x, base1_y);
    let _ = ctx.stroke();

    // Inner side of the head.
    ctx.move_to(tip_x, tip_y);
    ctx.line_to(base2_x, base2_y);
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

/// Draw a clear/trash icon
pub fn draw_icon_clear(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Trash can body
    ctx.move_to(x + s * 0.25, y + s * 0.35);
    ctx.line_to(x + s * 0.3, y + s * 0.8);
    ctx.line_to(x + s * 0.7, y + s * 0.8);
    ctx.line_to(x + s * 0.75, y + s * 0.35);
    let _ = ctx.stroke();

    // Lid
    ctx.move_to(x + s * 0.2, y + s * 0.35);
    ctx.line_to(x + s * 0.8, y + s * 0.35);
    let _ = ctx.stroke();

    // Handle
    ctx.move_to(x + s * 0.4, y + s * 0.35);
    ctx.line_to(x + s * 0.4, y + s * 0.25);
    ctx.line_to(x + s * 0.6, y + s * 0.25);
    ctx.line_to(x + s * 0.6, y + s * 0.35);
    let _ = ctx.stroke();
}

/// Draw a freeze/pause icon
pub fn draw_icon_freeze(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Snowflake - vertical line
    ctx.move_to(x + s * 0.5, y + s * 0.15);
    ctx.line_to(x + s * 0.5, y + s * 0.85);
    let _ = ctx.stroke();

    // Snowflake - diagonal lines
    ctx.move_to(x + s * 0.2, y + s * 0.32);
    ctx.line_to(x + s * 0.8, y + s * 0.68);
    let _ = ctx.stroke();

    ctx.move_to(x + s * 0.8, y + s * 0.32);
    ctx.line_to(x + s * 0.2, y + s * 0.68);
    let _ = ctx.stroke();

    // Small branches
    ctx.set_line_width(stroke * 0.7);
    ctx.move_to(x + s * 0.5, y + s * 0.3);
    ctx.line_to(x + s * 0.4, y + s * 0.2);
    ctx.move_to(x + s * 0.5, y + s * 0.3);
    ctx.line_to(x + s * 0.6, y + s * 0.2);
    let _ = ctx.stroke();
}

/// Draw an unfreeze/play icon
pub fn draw_icon_unfreeze(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    ctx.set_line_join(cairo::LineJoin::Round);

    // Play triangle
    ctx.move_to(x + s * 0.3, y + s * 0.2);
    ctx.line_to(x + s * 0.3, y + s * 0.8);
    ctx.line_to(x + s * 0.8, y + s * 0.5);
    ctx.close_path();
    let _ = ctx.fill();
}

/// Draw a settings/gear icon
pub fn draw_icon_settings(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    // Inner circle
    ctx.arc(cx, cy, s * 0.15, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Outer gear teeth (6 teeth)
    let inner_r = s * 0.25;
    let outer_r = s * 0.38;
    for i in 0..6 {
        let angle = (i as f64) * PI / 3.0;
        let x1 = cx + angle.cos() * inner_r;
        let y1 = cy + angle.sin() * inner_r;
        let x2 = cx + angle.cos() * outer_r;
        let y2 = cy + angle.sin() * outer_r;
        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        let _ = ctx.stroke();
    }

    // Outer circle
    ctx.arc(cx, cy, s * 0.32, 0.0, PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw a document/file icon
pub fn draw_icon_file(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Document outline with folded corner
    ctx.move_to(x + s * 0.25, y + s * 0.15);
    ctx.line_to(x + s * 0.6, y + s * 0.15);
    ctx.line_to(x + s * 0.75, y + s * 0.3);
    ctx.line_to(x + s * 0.75, y + s * 0.85);
    ctx.line_to(x + s * 0.25, y + s * 0.85);
    ctx.close_path();
    let _ = ctx.stroke();

    // Folded corner
    ctx.move_to(x + s * 0.6, y + s * 0.15);
    ctx.line_to(x + s * 0.6, y + s * 0.3);
    ctx.line_to(x + s * 0.75, y + s * 0.3);
    let _ = ctx.stroke();

    // Text lines
    ctx.move_to(x + s * 0.35, y + s * 0.5);
    ctx.line_to(x + s * 0.65, y + s * 0.5);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.35, y + s * 0.65);
    ctx.line_to(x + s * 0.65, y + s * 0.65);
    let _ = ctx.stroke();
}

/// Draw a minus icon
pub fn draw_icon_minus(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.15).max(2.0);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x + s * 0.25, y + s * 0.5);
    ctx.line_to(x + s * 0.75, y + s * 0.5);
    let _ = ctx.stroke();
}

/// Draw a plus icon
pub fn draw_icon_plus(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.15).max(2.0);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x + s * 0.25, y + s * 0.5);
    ctx.line_to(x + s * 0.75, y + s * 0.5);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.5, y + s * 0.25);
    ctx.line_to(x + s * 0.5, y + s * 0.75);
    let _ = ctx.stroke();
}

/// Draw a clock/delay icon
#[allow(dead_code)]
pub fn draw_icon_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    // Clock circle
    ctx.arc(cx, cy, s * 0.35, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Clock hands
    ctx.move_to(cx, cy);
    ctx.line_to(cx, cy - s * 0.2);
    let _ = ctx.stroke();
    ctx.move_to(cx, cy);
    ctx.line_to(cx + s * 0.15, cy + s * 0.1);
    let _ = ctx.stroke();
}
