//! Icon drawing functions for the toolbar UI.
//!
//! All icons are drawn using Cairo paths for perfect scaling at any DPI.

use cairo::Context;
use std::f64::consts::PI;

/// Draw a cursor/select icon (arrow pointer)
pub fn draw_icon_select(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Classic cursor arrow shape
    let start_x = x + s * 0.2;
    let start_y = y + s * 0.15;
    let height = s * 0.65;
    let mid_y = start_y + height * 0.6;

    // Outer path (filled)
    ctx.move_to(start_x, start_y);
    ctx.line_to(start_x, start_y + height);
    ctx.line_to(start_x + s * 0.2, mid_y);
    ctx.line_to(start_x + s * 0.35, start_y + height * 0.85);
    ctx.line_to(start_x + s * 0.45, start_y + height * 0.75);
    ctx.line_to(start_x + s * 0.3, mid_y - s * 0.1);
    ctx.line_to(start_x + s * 0.5, start_y + s * 0.1);
    ctx.close_path();

    let _ = ctx.fill_preserve();
    let _ = ctx.stroke();
}

/// Draw a pen/freehand icon (pencil with wavy stroke)
pub fn draw_icon_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Pencil body - angled rectangle
    let px = x + s * 0.55; // Pencil center x
    let py = y + s * 0.35; // Pencil center y
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
        x + s * 0.3,
        y + s * 0.62,
        x + s * 0.45,
        y + s * 0.82,
        x + s * 0.65,
        y + s * 0.72,
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

/// Draw a marker/highlighter icon (tilted marker with translucent swatch)
pub fn draw_icon_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.4);
    let swatch_color = (1.0, 0.92, 0.35, 0.4);

    // Underlying swatch to imply transparent ink
    ctx.set_source_rgba(
        swatch_color.0,
        swatch_color.1,
        swatch_color.2,
        swatch_color.3,
    );
    let swatch_h = s * 0.28;
    ctx.rectangle(x + s * 0.08, y + s * 0.65, s * 0.84, swatch_h);
    let _ = ctx.fill();

    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Marker body (tilted)
    ctx.save().ok();
    ctx.translate(x + s * 0.55, y + s * 0.35);
    ctx.rotate(-PI * 0.18);

    // Body outline
    ctx.set_source_rgba(0.95, 0.95, 0.98, 0.95);
    ctx.rectangle(-s * 0.24, -s * 0.08, s * 0.38, s * 0.26);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.25, 0.28, 0.35, 0.9);
    let _ = ctx.stroke();

    // Chisel tip with ink color
    ctx.set_source_rgba(swatch_color.0, swatch_color.1, swatch_color.2, 0.85);
    ctx.move_to(-s * 0.24, -s * 0.08);
    ctx.line_to(-s * 0.32, 0.0);
    ctx.line_to(-s * 0.24, s * 0.18);
    ctx.close_path();
    let _ = ctx.fill();
    ctx.restore().ok();
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

    // Angular separation between the two heads (~63°)
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
