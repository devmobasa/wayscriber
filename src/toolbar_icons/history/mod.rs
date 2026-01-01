mod arrows;
mod clock;
mod steps;

use cairo::Context;

/// Draw an undo icon (curved arrow left)
pub fn draw_icon_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    arrows::draw_curved_arrow(ctx, x, y, size, false);
}

/// Draw a redo icon (curved arrow right)
pub fn draw_icon_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    // Mirror the undo arrow for perfect symmetry
    with_horizontal_mirror(ctx, x, y, size, |ctx| {
        arrows::draw_curved_arrow(ctx, 0.0, 0.0, size, false);
    });
}

/// Draw an undo all icon (double curved arrow left)
pub fn draw_icon_undo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    arrows::draw_double_curved_arrow(ctx, x, y, size);
}

/// Draw a redo all icon (double curved arrow right)
pub fn draw_icon_redo_all(ctx: &Context, x: f64, y: f64, size: f64) {
    with_horizontal_mirror(ctx, x, y, size, |ctx| {
        arrows::draw_double_curved_arrow(ctx, 0.0, 0.0, size);
    });
}

/// Draw an undo all delay icon (double curved arrow left with clock)
pub fn draw_icon_undo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Double arrow
    arrows::draw_double_curved_arrow(ctx, x, y, s);

    // Small clock indicator at bottom-right, tucked into the corner
    let clock_r = s * 0.12;
    let clock_x = x + s * 0.83;
    let clock_y = y + s * 0.83;
    clock::draw_clock(ctx, clock_x, clock_y, clock_r);
}

/// Draw a redo all delay icon (double curved arrow right with clock)
pub fn draw_icon_redo_all_delay(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Mirror the double arrow
    with_horizontal_mirror(ctx, x, y, s, |ctx| {
        arrows::draw_double_curved_arrow(ctx, 0.0, 0.0, s);
    });

    // Small clock indicator at bottom-left
    let clock_r = s * 0.12;
    let clock_x = x + s * 0.17;
    let clock_y = y + s * 0.83;
    clock::draw_clock(ctx, clock_x, clock_y, clock_r);
}

/// Draw a step undo icon (curved arrow with number)
pub fn draw_icon_step_undo(ctx: &Context, x: f64, y: f64, size: f64) {
    steps::draw_step_undo(ctx, x, y, size);
}

/// Draw a step redo icon (curved arrow with number)
pub fn draw_icon_step_redo(ctx: &Context, x: f64, y: f64, size: f64) {
    steps::draw_step_redo(ctx, x, y, size);
}

fn with_horizontal_mirror(ctx: &Context, x: f64, y: f64, size: f64, draw: impl FnOnce(&Context)) {
    ctx.save().ok();
    ctx.translate(x + size, y);
    ctx.scale(-1.0, 1.0);
    draw(ctx);
    ctx.restore().ok();
}
