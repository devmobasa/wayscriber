//! Icon drawing functions for the toolbar UI.
//!
//! All icons are drawn using Cairo paths for perfect scaling at any DPI.

use cairo::Context;
use std::f64::consts::PI;

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
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow
    ctx.arc_negative(x + s * 0.5, y + s * 0.5, s * 0.3, PI * 0.2, PI * 1.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.2, y + s * 0.5);
    ctx.line_to(x + s * 0.3, y + s * 0.35);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.2, y + s * 0.5);
    ctx.line_to(x + s * 0.38, y + s * 0.55);
    let _ = ctx.stroke();
}

/// Draw a redo icon (curved arrow right)
pub fn draw_icon_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow
    ctx.arc(x + s * 0.5, y + s * 0.5, s * 0.3, PI * 0.8, PI * 0.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.8, y + s * 0.5);
    ctx.line_to(x + s * 0.7, y + s * 0.35);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.8, y + s * 0.5);
    ctx.line_to(x + s * 0.62, y + s * 0.55);
    let _ = ctx.stroke();
}

/// Draw an undo all icon (double curved arrow left)
pub fn draw_icon_undo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // First arrow
    ctx.arc_negative(x + s * 0.45, y + s * 0.5, s * 0.22, PI * 0.2, PI * 1.0);
    let _ = ctx.stroke();

    // Second arrow (behind)
    ctx.arc_negative(x + s * 0.6, y + s * 0.5, s * 0.22, PI * 0.2, PI * 1.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.23, y + s * 0.5);
    ctx.line_to(x + s * 0.32, y + s * 0.38);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.23, y + s * 0.5);
    ctx.line_to(x + s * 0.38, y + s * 0.54);
    let _ = ctx.stroke();
}

/// Draw a redo all icon (double curved arrow right)
pub fn draw_icon_redo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // First arrow
    ctx.arc(x + s * 0.55, y + s * 0.5, s * 0.22, PI * 0.8, PI * 0.0);
    let _ = ctx.stroke();

    // Second arrow (behind)
    ctx.arc(x + s * 0.4, y + s * 0.5, s * 0.22, PI * 0.8, PI * 0.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.77, y + s * 0.5);
    ctx.line_to(x + s * 0.68, y + s * 0.38);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.77, y + s * 0.5);
    ctx.line_to(x + s * 0.62, y + s * 0.54);
    let _ = ctx.stroke();
}

/// Draw an undo all delay icon (double curved arrow left with clock)
pub fn draw_icon_undo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // First arrow (slightly smaller and higher)
    ctx.arc_negative(x + s * 0.4, y + s * 0.4, s * 0.18, PI * 0.2, PI * 1.0);
    let _ = ctx.stroke();

    // Second arrow (behind)
    ctx.arc_negative(x + s * 0.52, y + s * 0.4, s * 0.18, PI * 0.2, PI * 1.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.22, y + s * 0.4);
    ctx.line_to(x + s * 0.30, y + s * 0.30);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.22, y + s * 0.4);
    ctx.line_to(x + s * 0.34, y + s * 0.44);
    let _ = ctx.stroke();

    // Small clock indicator at bottom right
    let clock_r = s * 0.14;
    let clock_x = x + s * 0.72;
    let clock_y = y + s * 0.72;
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

    // First arrow (slightly smaller and higher)
    ctx.arc(x + s * 0.6, y + s * 0.4, s * 0.18, PI * 0.8, PI * 0.0);
    let _ = ctx.stroke();

    // Second arrow (behind)
    ctx.arc(x + s * 0.48, y + s * 0.4, s * 0.18, PI * 0.8, PI * 0.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.78, y + s * 0.4);
    ctx.line_to(x + s * 0.70, y + s * 0.30);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.78, y + s * 0.4);
    ctx.line_to(x + s * 0.66, y + s * 0.44);
    let _ = ctx.stroke();

    // Small clock indicator at bottom left
    let clock_r = s * 0.14;
    let clock_x = x + s * 0.28;
    let clock_y = y + s * 0.72;
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

/// Draw a step undo icon (curved arrow with step indicator)
pub fn draw_icon_step_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow (positioned higher to make room for step bar)
    ctx.arc_negative(x + s * 0.5, y + s * 0.4, s * 0.25, PI * 0.2, PI * 1.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.25, y + s * 0.4);
    ctx.line_to(x + s * 0.35, y + s * 0.28);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.25, y + s * 0.4);
    ctx.line_to(x + s * 0.4, y + s * 0.45);
    let _ = ctx.stroke();

    // Step indicator bars at bottom
    ctx.set_line_width((s * 0.08).max(1.5));
    let bar_y = y + s * 0.78;
    ctx.move_to(x + s * 0.25, bar_y);
    ctx.line_to(x + s * 0.4, bar_y);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.45, bar_y);
    ctx.line_to(x + s * 0.6, bar_y);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.65, bar_y);
    ctx.line_to(x + s * 0.75, bar_y);
    let _ = ctx.stroke();
}

/// Draw a step redo icon (curved arrow with step indicator)
pub fn draw_icon_step_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Curved arrow (positioned higher to make room for step bar)
    ctx.arc(x + s * 0.5, y + s * 0.4, s * 0.25, PI * 0.8, PI * 0.0);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.75, y + s * 0.4);
    ctx.line_to(x + s * 0.65, y + s * 0.28);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.75, y + s * 0.4);
    ctx.line_to(x + s * 0.6, y + s * 0.45);
    let _ = ctx.stroke();

    // Step indicator bars at bottom
    ctx.set_line_width((s * 0.08).max(1.5));
    let bar_y = y + s * 0.78;
    ctx.move_to(x + s * 0.25, bar_y);
    ctx.line_to(x + s * 0.4, bar_y);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.45, bar_y);
    ctx.line_to(x + s * 0.6, bar_y);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.65, bar_y);
    ctx.line_to(x + s * 0.75, bar_y);
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
