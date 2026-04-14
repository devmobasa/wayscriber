use cairo::Context;

/// Draw a cursor/select icon (arrow pointer)
pub fn draw_icon_select(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_select(ctx, x, y, size);
}

/// Draw a pen/freehand icon (nib with a short stroke)
pub fn draw_icon_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_pen(ctx, x, y, size);
}

/// Draw a line tool icon
pub fn draw_icon_line(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_line(ctx, x, y, size);
}

/// Draw a rectangle tool icon
pub fn draw_icon_rect(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_rect(ctx, x, y, size);
}

/// Draw a circle/ellipse tool icon
pub fn draw_icon_circle(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_circle(ctx, x, y, size);
}

/// Draw an arrow tool icon
pub fn draw_icon_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_arrow(ctx, x, y, size);
}

/// Draw a blur tool icon
pub fn draw_icon_blur(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_blur(ctx, x, y, size);
}

/// Draw an eraser tool icon
#[allow(dead_code)]
pub fn draw_icon_eraser(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_eraser(ctx, x, y, size);
}

/// Draw a text tool icon (letter T)
pub fn draw_icon_text(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_text(ctx, x, y, size);
}

/// Draw a sticky note icon (square with folded corner)
pub fn draw_icon_note(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_note(ctx, x, y, size);
}

/// Draw a highlighter tool icon (cursor with click ripple effect)
#[allow(dead_code)]
pub fn draw_icon_highlight(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_highlight(ctx, x, y, size);
}

/// Draw a marker/highlighter icon
pub fn draw_icon_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_marker(ctx, x, y, size);
}

/// Draw a step marker icon (numbered list)
pub fn draw_icon_step_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_step_marker(ctx, x, y, size);
}
