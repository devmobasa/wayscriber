//! Drop-in replacement for `src/toolbar_icons/tools.rs` wrappers.
//! Geometry and current-source inheritance live in `super::svg`.

use cairo::Context;

pub fn draw_icon_select(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_select(ctx, x, y, size);
}

pub fn draw_icon_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_pen(ctx, x, y, size);
}

pub fn draw_icon_line(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_line(ctx, x, y, size);
}

pub fn draw_icon_rect(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_rect(ctx, x, y, size);
}

pub fn draw_icon_circle(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_circle(ctx, x, y, size);
}

pub fn draw_icon_triangle(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_triangle(ctx, x, y, size);
}

pub fn draw_icon_parallelogram(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_parallelogram(ctx, x, y, size);
}

pub fn draw_icon_rhombus(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_rhombus(ctx, x, y, size);
}

pub fn draw_icon_polygon(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_polygon(ctx, x, y, size);
}

pub fn draw_icon_freeform_polygon(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_freeform_polygon(ctx, x, y, size);
}

pub fn draw_icon_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_arrow(ctx, x, y, size);
}

pub fn draw_icon_blur(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_blur(ctx, x, y, size);
}

pub fn draw_icon_eraser(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_eraser(ctx, x, y, size);
}

pub fn draw_icon_text(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_text(ctx, x, y, size);
}

pub fn draw_icon_note(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_note(ctx, x, y, size);
}

pub fn draw_icon_highlight(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_highlight(ctx, x, y, size);
}

pub fn draw_icon_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_marker(ctx, x, y, size);
}

pub fn draw_icon_step_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_step_marker(ctx, x, y, size);
}

/// Draw the shape-palette opener. This represents the family of shapes rather
/// than any one currently selected shape tool.
pub fn draw_icon_shape_picker(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_shape_picker(ctx, x, y, size);
}

/// Draw the fill affordance used by shape options.
#[allow(dead_code)] // reserved for the shape-options toolbar surface
pub fn draw_icon_fill(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_fill(ctx, x, y, size);
}

/// Draw a highlight/ring affordance for option surfaces.
#[allow(dead_code)] // reserved for the highlight-options toolbar surface
pub fn draw_icon_highlight_ring(ctx: &Context, x: f64, y: f64, size: f64) {
    super::svg::render_highlight_ring(ctx, x, y, size);
}
