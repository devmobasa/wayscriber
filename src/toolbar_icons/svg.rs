//! Tool icon drawing via local Cairo paths.
//!
//! These icons intentionally mirror the small Lucide-style assets used by the
//! toolbar without depending on a full SVG parser and rasterizer.

use cairo::Context;
use std::f64::consts::PI;

type IconDraw = fn(&Context);

fn render_icon(ctx: &Context, x: f64, y: f64, size: f64, draw: IconDraw) {
    if size <= 0.0 {
        return;
    }

    let _ = ctx.save();
    ctx.translate(x, y);
    let scale = size / 24.0;
    ctx.scale(scale, scale);
    ctx.set_line_width(2.0);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    draw(ctx);
    let _ = ctx.restore();
}

fn stroke(ctx: &Context) {
    let _ = ctx.stroke();
}

fn fill(ctx: &Context) {
    let _ = ctx.fill();
}

fn rounded_rect(ctx: &Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    ctx.new_path();
    ctx.arc(x + width - radius, y + radius, radius, -PI / 2.0, 0.0);
    ctx.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        PI / 2.0,
    );
    ctx.arc(x + radius, y + height - radius, radius, PI / 2.0, PI);
    ctx.arc(x + radius, y + radius, radius, PI, 3.0 * PI / 2.0);
    ctx.close_path();
}

fn circle(ctx: &Context, x: f64, y: f64, radius: f64) {
    ctx.new_path();
    ctx.arc(x, y, radius, 0.0, PI * 2.0);
}

fn draw_select(ctx: &Context) {
    ctx.move_to(12.6, 12.6);
    ctx.line_to(19.0, 19.0);
    stroke(ctx);

    ctx.move_to(3.7, 3.0);
    ctx.line_to(10.2, 19.0);
    ctx.line_to(12.0, 13.5);
    ctx.line_to(19.6, 10.5);
    ctx.close_path();
    stroke(ctx);
}

fn draw_pen(ctx: &Context) {
    ctx.move_to(15.7, 21.3);
    ctx.line_to(12.7, 18.3);
    ctx.line_to(18.3, 12.7);
    ctx.line_to(21.3, 15.7);
    ctx.close_path();
    stroke(ctx);

    ctx.move_to(18.0, 13.0);
    ctx.line_to(16.6, 6.1);
    ctx.line_to(2.3, 2.3);
    ctx.line_to(6.1, 16.6);
    ctx.line_to(13.0, 18.0);
    stroke(ctx);

    ctx.move_to(2.3, 2.3);
    ctx.line_to(9.6, 9.6);
    stroke(ctx);

    circle(ctx, 11.0, 11.0, 2.0);
    stroke(ctx);
}

fn draw_line(ctx: &Context) {
    ctx.move_to(5.0, 12.0);
    ctx.line_to(19.0, 12.0);
    stroke(ctx);
}

fn draw_rect(ctx: &Context) {
    rounded_rect(ctx, 2.0, 6.0, 20.0, 12.0, 2.0);
    stroke(ctx);
}

fn draw_circle(ctx: &Context) {
    circle(ctx, 12.0, 12.0, 10.0);
    stroke(ctx);
}

fn draw_arrow(ctx: &Context) {
    ctx.move_to(7.0, 7.0);
    ctx.line_to(17.0, 7.0);
    ctx.line_to(17.0, 17.0);
    stroke(ctx);

    ctx.move_to(7.0, 17.0);
    ctx.line_to(17.0, 7.0);
    stroke(ctx);
}

fn draw_blur(ctx: &Context) {
    rounded_rect(ctx, 4.0, 4.0, 16.0, 16.0, 3.0);
    stroke(ctx);

    for (x, y, radius) in [
        (9.0, 9.0, 1.2),
        (15.0, 9.0, 1.2),
        (12.0, 12.0, 1.6),
        (9.0, 15.0, 1.2),
        (15.0, 15.0, 1.2),
    ] {
        circle(ctx, x, y, radius);
        fill(ctx);
    }
}

fn draw_eraser(ctx: &Context) {
    ctx.move_to(21.0, 21.0);
    ctx.line_to(8.0, 21.0);
    ctx.line_to(2.6, 15.6);
    ctx.line_to(13.3, 4.9);
    ctx.line_to(20.7, 12.3);
    ctx.line_to(12.8, 21.0);
    stroke(ctx);

    ctx.move_to(5.1, 11.1);
    ctx.line_to(13.9, 19.9);
    stroke(ctx);
}

fn draw_text(ctx: &Context) {
    ctx.move_to(12.0, 4.0);
    ctx.line_to(12.0, 20.0);
    stroke(ctx);

    ctx.move_to(4.0, 7.0);
    ctx.line_to(4.0, 5.0);
    ctx.line_to(20.0, 5.0);
    ctx.line_to(20.0, 7.0);
    stroke(ctx);

    ctx.move_to(9.0, 20.0);
    ctx.line_to(15.0, 20.0);
    stroke(ctx);
}

fn draw_note(ctx: &Context) {
    ctx.move_to(21.0, 9.0);
    ctx.line_to(21.0, 19.0);
    ctx.line_to(19.0, 21.0);
    ctx.line_to(5.0, 21.0);
    ctx.line_to(3.0, 19.0);
    ctx.line_to(3.0, 5.0);
    ctx.line_to(5.0, 3.0);
    ctx.line_to(15.0, 3.0);
    ctx.line_to(21.0, 9.0);
    stroke(ctx);

    ctx.move_to(15.0, 3.0);
    ctx.line_to(15.0, 8.0);
    ctx.line_to(16.0, 9.0);
    ctx.line_to(21.0, 9.0);
    stroke(ctx);
}

fn draw_highlight(ctx: &Context) {
    for (x1, y1, x2, y2) in [
        (14.0, 4.1, 12.0, 6.0),
        (5.1, 8.0, 2.2, 7.2),
        (6.0, 12.0, 4.1, 14.0),
        (7.2, 2.2, 8.0, 5.1),
    ] {
        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        stroke(ctx);
    }

    ctx.move_to(9.0, 9.7);
    ctx.line_to(20.7, 14.0);
    ctx.line_to(16.3, 15.5);
    ctx.line_to(14.5, 20.7);
    ctx.close_path();
    stroke(ctx);
}

fn draw_marker(ctx: &Context) {
    ctx.move_to(9.0, 11.0);
    ctx.line_to(3.0, 17.0);
    ctx.line_to(3.0, 20.0);
    ctx.line_to(12.0, 20.0);
    ctx.line_to(15.0, 17.0);
    stroke(ctx);

    ctx.move_to(22.0, 12.0);
    ctx.line_to(17.4, 16.6);
    ctx.line_to(14.6, 16.6);
    ctx.line_to(9.4, 11.4);
    ctx.line_to(9.4, 8.6);
    ctx.line_to(14.0, 4.0);
    stroke(ctx);
}

fn draw_step_marker(ctx: &Context) {
    for y in [5.0, 12.0, 19.0] {
        ctx.move_to(11.0, y);
        ctx.line_to(21.0, y);
        stroke(ctx);
    }

    ctx.move_to(4.0, 4.0);
    ctx.line_to(5.0, 4.0);
    ctx.line_to(5.0, 9.0);
    stroke(ctx);

    ctx.move_to(4.0, 9.0);
    ctx.line_to(6.0, 9.0);
    stroke(ctx);

    ctx.move_to(3.4, 15.5);
    ctx.curve_to(4.0, 14.5, 6.5, 14.7, 6.5, 16.5);
    ctx.curve_to(6.5, 18.1, 3.9, 19.0, 3.4, 20.0);
    ctx.line_to(6.5, 20.0);
    stroke(ctx);
}

pub fn render_select(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_select);
}

pub fn render_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_pen);
}

pub fn render_line(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_line);
}

pub fn render_rect(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_rect);
}

pub fn render_circle(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_circle);
}

pub fn render_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_arrow);
}

pub fn render_blur(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_blur);
}

pub fn render_eraser(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_eraser);
}

pub fn render_text(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_text);
}

pub fn render_note(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_note);
}

pub fn render_highlight(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_highlight);
}

pub fn render_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_marker);
}

pub fn render_step_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    render_icon(ctx, x, y, size, draw_step_marker);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Format, ImageSurface};

    type IconRenderFn = fn(&Context, f64, f64, f64);

    fn assert_icon_renders(name: &str, draw: IconRenderFn) {
        let surface = ImageSurface::create(Format::ARgb32, 24, 24).expect("surface");
        let ctx = Context::new(&surface).expect("context");
        ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
        draw(&ctx, 0.0, 0.0, 24.0);
        surface.flush();

        let mut has_ink = false;
        surface
            .with_data(|pixels| {
                has_ink = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
            })
            .expect("surface data");
        assert!(has_ink, "{name} icon rendered an empty surface");
    }

    #[test]
    fn tool_icons_render_non_empty_alpha() {
        let icons: [(&str, IconRenderFn); 13] = [
            ("select", render_select),
            ("pen", render_pen),
            ("line", render_line),
            ("rect", render_rect),
            ("circle", render_circle),
            ("arrow", render_arrow),
            ("blur", render_blur),
            ("eraser", render_eraser),
            ("text", render_text),
            ("note", render_note),
            ("highlight", render_highlight),
            ("marker", render_marker),
            ("step_marker", render_step_marker),
        ];

        for (name, draw) in icons {
            assert_icon_renders(name, draw);
        }
    }
}
