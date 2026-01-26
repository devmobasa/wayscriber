use super::constants::{
    COLOR_BUTTON_ACTIVE, COLOR_BUTTON_DEFAULT, COLOR_BUTTON_HOVER, COLOR_CLOSE_DEFAULT,
    COLOR_CLOSE_HOVER, COLOR_FOCUS_RING, COLOR_PIN_ACTIVE, COLOR_PIN_DEFAULT, COLOR_PIN_HOVER,
    COLOR_TEXT_PRIMARY, LINE_WIDTH_THICK, RADIUS_LG, RADIUS_STD, SPACING_XS, set_color,
};
use super::draw_round_rect;
use std::f64::consts::PI;

pub(in crate::backend::wayland::toolbar::render) fn draw_drag_handle(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    hover: bool,
) {
    draw_round_rect(ctx, x, y, w, h, RADIUS_STD);
    // Improved visibility: higher fill alpha
    let fill_alpha = if hover { 0.75 } else { 0.45 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, fill_alpha);
    let _ = ctx.fill();

    // Add subtle glow on hover
    if hover {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.15);
        draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_STD + 1.0);
        let _ = ctx.stroke();
    }

    ctx.set_line_width(1.1);
    let bar_alpha = if hover { 1.0 } else { 0.85 };
    ctx.set_source_rgba(1.0, 1.0, 1.0, bar_alpha);
    let bar_w = w * 0.55;
    let bar_h = SPACING_XS;
    let bar_x = x + (w - bar_w) / 2.0;
    let mut bar_y = y + (h - 3.0 * bar_h) / 2.0;
    for _ in 0..3 {
        draw_round_rect(ctx, bar_x, bar_y, bar_w, bar_h, 1.0);
        let _ = ctx.fill();
        bar_y += bar_h + SPACING_XS;
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_close_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    hover: bool,
) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    set_color(
        ctx,
        if hover {
            COLOR_CLOSE_HOVER
        } else {
            COLOR_CLOSE_DEFAULT
        },
    );
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    set_color(ctx, COLOR_TEXT_PRIMARY);
    ctx.set_line_width(LINE_WIDTH_THICK);
    let inset = size * 0.3;
    ctx.move_to(x + inset, y + inset);
    ctx.line_to(x + size - inset, y + size - inset);
    let _ = ctx.stroke();
    ctx.move_to(x + size - inset, y + inset);
    ctx.line_to(x + inset, y + size - inset);
    let _ = ctx.stroke();
}

pub(in crate::backend::wayland::toolbar::render) fn draw_pin_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    size: f64,
    pinned: bool,
    hover: bool,
) {
    let r = size / 2.0;
    let cx = x + r;
    let cy = y + r;

    // Draw outer glow when pinned for visual feedback
    if pinned {
        ctx.set_source_rgba(0.4, 0.65, 1.0, 0.3);
        ctx.arc(cx, cy, r + 2.0, 0.0, PI * 2.0);
        let _ = ctx.fill();
    }

    // Use circle shape for visual consistency with close button
    let color = if pinned {
        COLOR_PIN_ACTIVE
    } else if hover {
        COLOR_PIN_HOVER
    } else {
        COLOR_PIN_DEFAULT
    };
    set_color(ctx, color);
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.fill();

    // Draw star icon instead of pin
    let star_size = size * 0.45;
    draw_star(ctx, cx, cy, star_size, pinned);
}

/// Draw a 5-pointed star centered at (cx, cy)
fn draw_star(ctx: &cairo::Context, cx: f64, cy: f64, size: f64, filled: bool) {
    let outer_r = size;
    let inner_r = size * 0.4;
    let points = 5;

    ctx.new_path();
    for i in 0..(points * 2) {
        let angle = (i as f64) * PI / (points as f64) - PI / 2.0;
        let r = if i % 2 == 0 { outer_r } else { inner_r };
        let px = cx + r * angle.cos();
        let py = cy + r * angle.sin();
        if i == 0 {
            ctx.move_to(px, py);
        } else {
            ctx.line_to(px, py);
        }
    }
    ctx.close_path();

    if filled {
        // Filled star uses primary text color for theme consistency.
        set_color(ctx, COLOR_TEXT_PRIMARY);
        let _ = ctx.fill();
    } else {
        // Outline star when not pinned
        set_color(ctx, COLOR_TEXT_PRIMARY);
        ctx.set_line_width(1.5);
        let _ = ctx.stroke();
    }
}

pub(in crate::backend::wayland::toolbar::render) fn draw_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    active: bool,
    hover: bool,
) {
    // Add subtle glow on hover for better visibility
    if hover && !active {
        ctx.set_source_rgba(1.0, 1.0, 1.0, 0.08);
        draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_LG + 1.0);
        let _ = ctx.fill();
    }

    // Active state: add outer glow ring
    if active {
        ctx.set_source_rgba(0.3, 0.55, 1.0, 0.25);
        draw_round_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, RADIUS_LG + 2.0);
        let _ = ctx.fill();
    }

    let color = if active {
        COLOR_BUTTON_ACTIVE
    } else if hover {
        COLOR_BUTTON_HOVER
    } else {
        COLOR_BUTTON_DEFAULT
    };
    set_color(ctx, color);
    draw_round_rect(ctx, x, y, w, h, RADIUS_LG);
    let _ = ctx.fill();

    // Active state: add bottom indicator line
    if active {
        ctx.set_source_rgba(0.5, 0.75, 1.0, 0.95);
        let indicator_w = w * 0.5;
        let indicator_h = 2.5;
        let indicator_x = x + (w - indicator_w) / 2.0;
        let indicator_y = y + h - indicator_h - 2.0;
        draw_round_rect(ctx, indicator_x, indicator_y, indicator_w, indicator_h, 1.5);
        let _ = ctx.fill();
    }
}

/// Draw a keyboard focus ring around an element.
/// Call this after drawing the element to show focus indication.
#[allow(dead_code)]
pub(in crate::backend::wayland::toolbar::render) fn draw_focus_ring(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius: f64,
) {
    set_color(ctx, COLOR_FOCUS_RING);
    ctx.set_line_width(2.0);
    draw_round_rect(ctx, x - 2.0, y - 2.0, w + 4.0, h + 4.0, radius + 2.0);
    let _ = ctx.stroke();
}

/// Draw a button with a subtle warning accent for destructive actions (e.g., Clear, UndoAll).
pub(in crate::backend::wayland::toolbar::render) fn draw_destructive_button(
    ctx: &cairo::Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    hover: bool,
) {
    // Add warning-tinted glow on hover
    if hover {
        ctx.set_source_rgba(0.9, 0.4, 0.3, 0.15);
        draw_round_rect(ctx, x - 1.0, y - 1.0, w + 2.0, h + 2.0, RADIUS_LG + 1.0);
        let _ = ctx.fill();
    }

    let color = if hover {
        COLOR_BUTTON_HOVER
    } else {
        COLOR_BUTTON_DEFAULT
    };
    set_color(ctx, color);
    draw_round_rect(ctx, x, y, w, h, RADIUS_LG);
    let _ = ctx.fill();

    // Subtle red accent line at top edge
    ctx.set_source_rgba(0.85, 0.35, 0.3, if hover { 0.8 } else { 0.5 });
    let accent_w = w * 0.6;
    let accent_h = 2.0;
    let accent_x = x + (w - accent_w) / 2.0;
    let accent_y = y + 2.0;
    draw_round_rect(ctx, accent_x, accent_y, accent_w, accent_h, 1.0);
    let _ = ctx.fill();
}
