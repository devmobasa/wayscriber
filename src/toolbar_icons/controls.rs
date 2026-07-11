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
    super::svg::render_more(ctx, x, y, size);
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
    draw_icon_drag(ctx, x, y, size);
}

/// Draw the toolbar drag handle.
pub fn draw_icon_drag(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_drag(ctx, x, y, size);
}

/// Draw a minimize dash (not an X: the bar collapses to a restore tab).
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_dash(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_icon_minimize(ctx, x, y, size);
}

/// Draw a pushpin glyph ("keep open at startup"): outline when unpinned,
/// filled when pinned. Geometry mirrors the built-in pin chrome button.
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_pushpin(ctx: &Context, x: f64, y: f64, size: f64, filled: bool) {
    if filled {
        draw_icon_pin(ctx, x, y, size);
    } else {
        draw_icon_unpin(ctx, x, y, size);
    }
}

/// Outline pushpin with the 4-arg painter shape used by icon widgets.
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_pin_outline(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_icon_unpin(ctx, x, y, size);
}

/// Filled pushpin with the 4-arg painter shape used by icon widgets.
#[allow(dead_code)] // referenced by the toolbar-gtk frontend only
pub fn draw_icon_pin_filled(ctx: &Context, x: f64, y: f64, size: f64) {
    draw_icon_pin(ctx, x, y, size);
}

pub fn draw_icon_pin(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_pin(ctx, x, y, size);
}

pub fn draw_icon_unpin(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_unpin(ctx, x, y, size);
}

pub fn draw_icon_minimize(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_minimize(ctx, x, y, size);
}

pub fn draw_icon_side_minimize(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_side_minimize(ctx, x, y, size);
}

pub fn draw_icon_restore(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_restore(ctx, x, y, size);
}

#[allow(dead_code)] // part of the complete icon family; no close action today
pub fn draw_icon_close(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_close(ctx, x, y, size);
}
