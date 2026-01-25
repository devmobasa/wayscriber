use cairo::Context;
use std::f64::consts::PI;

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

/// Draw a "more" (three dots) icon
pub fn draw_icon_more(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let r = (s * 0.09).max(1.5);
    let cy = y + s * 0.5;
    let start_x = x + s * 0.25;
    let gap = s * 0.25;

    for i in 0..3 {
        let cx = start_x + gap * i as f64;
        ctx.arc(cx, cy, r, 0.0, PI * 2.0);
        let _ = ctx.fill();
    }
}

/// Draw a paste/clipboard icon
pub fn draw_icon_paste(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.2);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Draw clipboard outline (rounded rectangle)
    let clip_x = x + s * 0.2;
    let clip_y = y + s * 0.15;
    let clip_w = s * 0.6;
    let clip_h = s * 0.7;
    let r = s * 0.08;

    // Clipboard body
    ctx.new_path();
    ctx.arc(clip_x + clip_w - r, clip_y + r, r, -PI * 0.5, 0.0);
    ctx.arc(clip_x + clip_w - r, clip_y + clip_h - r, r, 0.0, PI * 0.5);
    ctx.arc(clip_x + r, clip_y + clip_h - r, r, PI * 0.5, PI);
    ctx.arc(clip_x + r, clip_y + r, r, PI, PI * 1.5);
    ctx.close_path();
    let _ = ctx.stroke();

    // Draw clip at top (small rectangle)
    let tab_w = s * 0.25;
    let tab_h = s * 0.12;
    let tab_x = x + (s - tab_w) / 2.0;
    let tab_y = y + s * 0.08;
    ctx.rectangle(tab_x, tab_y, tab_w, tab_h);
    let _ = ctx.fill();

    // Draw lines on clipboard (content)
    ctx.set_line_width(stroke * 0.8);
    let line_y1 = clip_y + clip_h * 0.35;
    let line_y2 = clip_y + clip_h * 0.55;
    let line_y3 = clip_y + clip_h * 0.75;
    let line_x1 = clip_x + s * 0.1;
    let line_x2 = clip_x + clip_w - s * 0.1;

    ctx.move_to(line_x1, line_y1);
    ctx.line_to(line_x2, line_y1);
    let _ = ctx.stroke();

    ctx.move_to(line_x1, line_y2);
    ctx.line_to(line_x2 - s * 0.1, line_y2);
    let _ = ctx.stroke();

    ctx.move_to(line_x1, line_y3);
    ctx.line_to(line_x2 - s * 0.2, line_y3);
    let _ = ctx.stroke();
}
