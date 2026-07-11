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

/// Draw an information icon.
pub fn draw_icon_info(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.4);
    let cx = x + s * 0.5;
    let cy = y + s * 0.5;

    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.arc(cx, cy, s * 0.38, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    ctx.arc(cx, y + s * 0.32, (s * 0.055).max(1.0), 0.0, PI * 2.0);
    let _ = ctx.fill();

    ctx.move_to(cx, y + s * 0.45);
    ctx.line_to(cx, y + s * 0.68);
    let _ = ctx.stroke();
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

/// Draw a simple board/grid icon
pub fn draw_icon_board(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.2);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let inset = s * 0.18;
    let x0 = x + inset;
    let y0 = y + inset;
    let w = s - inset * 2.0;
    let h = s - inset * 2.0;

    ctx.rectangle(x0, y0, w, h);
    let _ = ctx.stroke();

    ctx.move_to(x0 + w * 0.5, y0);
    ctx.line_to(x0 + w * 0.5, y0 + h);
    let _ = ctx.stroke();

    ctx.move_to(x0, y0 + h * 0.5);
    ctx.line_to(x0 + w, y0 + h * 0.5);
    let _ = ctx.stroke();
}

/// Draw a drag-grip glyph: three horizontal bars, as on the toolbar move
/// handles. Uses the context's current source color.
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_grip_bars(ctx: &Context, x: f64, y: f64, size: f64) {
    let bar_w = size * 0.55;
    let bar_h = (size * 0.11).max(1.5);
    let bar_gap = bar_h;
    let stack_h = 3.0 * bar_h + 2.0 * bar_gap;
    let bar_x = x + (size - bar_w) / 2.0;
    let mut bar_y = y + (size - stack_h) / 2.0;
    for _ in 0..3 {
        ctx.rectangle(bar_x, bar_y, bar_w, bar_h);
        let _ = ctx.fill();
        bar_y += bar_h + bar_gap;
    }
}

/// Draw a minimize dash (not an X: the bar collapses to a restore tab).
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_dash(ctx: &Context, x: f64, y: f64, size: f64) {
    ctx.set_line_width((size * 0.12).max(1.6));
    ctx.set_line_cap(cairo::LineCap::Round);
    let inset = size * 0.24;
    let cy = y + size / 2.0;
    ctx.move_to(x + inset, cy);
    ctx.line_to(x + size - inset, cy);
    let _ = ctx.stroke();
}

/// Draw a pushpin glyph ("keep open at startup"): outline when unpinned,
/// filled when pinned. Geometry mirrors the built-in pin chrome button.
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_pushpin(ctx: &Context, x: f64, y: f64, size: f64, filled: bool) {
    let s = size / 2.0;
    let cx = x + s;
    let cy = y + s;

    ctx.new_path();
    ctx.move_to(cx - s * 0.45, cy - s * 0.85);
    ctx.line_to(cx + s * 0.45, cy - s * 0.85);
    ctx.line_to(cx + s * 0.3, cy - s * 0.55);
    ctx.line_to(cx + s * 0.3, cy - s * 0.15);
    ctx.line_to(cx + s * 0.6, cy + s * 0.15);
    ctx.line_to(cx - s * 0.6, cy + s * 0.15);
    ctx.line_to(cx - s * 0.3, cy - s * 0.15);
    ctx.line_to(cx - s * 0.3, cy - s * 0.55);
    ctx.close_path();
    if filled {
        let _ = ctx.fill();
    } else {
        ctx.set_line_width(1.3);
        let _ = ctx.stroke();
    }

    ctx.set_line_width(if filled { 1.6 } else { 1.3 });
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.move_to(cx, cy + s * 0.15);
    ctx.line_to(cx, cy + s * 0.85);
    let _ = ctx.stroke();
}

/// Outline pushpin with the 4-arg painter shape used by icon widgets.
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_pin_outline(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_icon_pushpin(ctx, x, y, size, false);
}

/// Filled pushpin with the 4-arg painter shape used by icon widgets.
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_pin_filled(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_icon_pushpin(ctx, x, y, size, true);
}
