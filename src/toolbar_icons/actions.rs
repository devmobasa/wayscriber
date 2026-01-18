use cairo::Context;
use std::f64::consts::PI;

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

/// Draw a floppy disk/save icon
pub fn draw_icon_save(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    let pad = s * 0.15;
    let body_x = x + pad;
    let body_y = y + pad;
    let body_w = s - pad * 2.0;
    let body_h = s - pad * 2.0;
    let notch_w = s * 0.22;
    let notch_h = s * 0.18;

    // Outer body with a top-right notch.
    ctx.move_to(body_x, body_y);
    ctx.line_to(body_x + body_w - notch_w, body_y);
    ctx.line_to(body_x + body_w, body_y + notch_h);
    ctx.line_to(body_x + body_w, body_y + body_h);
    ctx.line_to(body_x, body_y + body_h);
    ctx.close_path();
    let _ = ctx.stroke();

    // Shutter tab near the top.
    let shutter_w = body_w * 0.4;
    let shutter_h = body_h * 0.18;
    let shutter_x = body_x + body_w * 0.1;
    let shutter_y = body_y + body_h * 0.12;
    ctx.rectangle(shutter_x, shutter_y, shutter_w, shutter_h);
    let _ = ctx.stroke();

    // Label window.
    let label_w = body_w * 0.55;
    let label_h = body_h * 0.22;
    let label_x = body_x + (body_w - label_w) / 2.0;
    let label_y = body_y + body_h - label_h - body_h * 0.12;
    ctx.rectangle(label_x, label_y, label_w, label_h);
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

/// Draw a refresh/reload icon (circular arrow).
#[allow(dead_code)]
pub fn draw_icon_refresh(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.11).max(1.6);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;
    let r = s * 0.32;
    let start = 0.25 * PI;
    let end = 1.9 * PI;

    ctx.arc(cx, cy, r, start, end);
    let _ = ctx.stroke();

    let arrow_x = cx + r * end.cos();
    let arrow_y = cy + r * end.sin();
    let head = s * 0.16;
    let left = end + 0.7;
    let right = end - 0.7;
    ctx.move_to(arrow_x, arrow_y);
    ctx.line_to(arrow_x - head * left.cos(), arrow_y - head * left.sin());
    ctx.line_to(arrow_x - head * right.cos(), arrow_y - head * right.sin());
    ctx.close_path();
    let _ = ctx.fill();
}

/// Draw a copy/duplicate icon (two overlapping rectangles).
pub fn draw_icon_copy(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Back rectangle
    ctx.rectangle(x + s * 0.3, y + s * 0.15, s * 0.5, s * 0.55);
    let _ = ctx.stroke();

    // Front rectangle (overlapping)
    ctx.rectangle(x + s * 0.2, y + s * 0.3, s * 0.5, s * 0.55);
    let _ = ctx.stroke();
}

/// Draw a left chevron/arrow icon for navigation.
pub fn draw_icon_chevron_left(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Chevron pointing left: >
    ctx.move_to(x + s * 0.6, y + s * 0.2);
    ctx.line_to(x + s * 0.35, y + s * 0.5);
    ctx.line_to(x + s * 0.6, y + s * 0.8);
    let _ = ctx.stroke();
}

/// Draw a right chevron/arrow icon for navigation.
pub fn draw_icon_chevron_right(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Chevron pointing right: <
    ctx.move_to(x + s * 0.4, y + s * 0.2);
    ctx.line_to(x + s * 0.65, y + s * 0.5);
    ctx.line_to(x + s * 0.4, y + s * 0.8);
    let _ = ctx.stroke();
}

/// Draw a pencil/edit icon.
#[allow(dead_code)]
pub fn draw_icon_pencil(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Pencil body (diagonal)
    ctx.move_to(x + s * 0.7, y + s * 0.15);
    ctx.line_to(x + s * 0.85, y + s * 0.3);
    ctx.line_to(x + s * 0.3, y + s * 0.85);
    ctx.line_to(x + s * 0.15, y + s * 0.7);
    ctx.close_path();
    let _ = ctx.stroke();

    // Tip line
    ctx.move_to(x + s * 0.15, y + s * 0.7);
    ctx.line_to(x + s * 0.3, y + s * 0.85);
    let _ = ctx.stroke();

    // Eraser line
    ctx.move_to(x + s * 0.6, y + s * 0.25);
    ctx.line_to(x + s * 0.75, y + s * 0.4);
    let _ = ctx.stroke();
}
