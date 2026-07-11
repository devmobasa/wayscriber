//! Wayscriber Core toolbar icons.
//!
//! Original MIT-licensed 24×24 geometry. The caller owns foreground color,
//! alpha, clipping, hover/active backgrounds, and disabled-state opacity.
//! Every public renderer accepts the same `(ctx, x, y, size)` contract used by
//! Wayscriber's existing Cairo toolbar icon entry points.

use cairo::Context;
use std::f64::consts::PI;

type IconDraw = fn(&Context);

fn render_icon(ctx: &Context, x: f64, y: f64, size: f64, draw: IconDraw) {
    if !size.is_finite() || size <= 0.0 {
        return;
    }
    let _ = ctx.save();
    ctx.translate(x, y);
    ctx.scale(size / 24.0, size / 24.0);
    ctx.set_line_width(2.0);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);
    draw(ctx);
    let _ = ctx.restore();
}

#[inline]
fn stroke(ctx: &Context) {
    let _ = ctx.stroke();
}

#[inline]
fn fill(ctx: &Context) {
    let _ = ctx.fill();
}

fn circle(ctx: &Context, cx: f64, cy: f64, radius: f64) {
    ctx.new_path();
    ctx.arc(cx, cy, radius, 0.0, PI * 2.0);
}

fn dot(ctx: &Context, cx: f64, cy: f64, radius: f64) {
    circle(ctx, cx, cy, radius);
    fill(ctx);
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
    ctx.arc(x + radius, y + radius, radius, PI, PI * 1.5);
    ctx.close_path();
}

fn ellipse(ctx: &Context, cx: f64, cy: f64, rx: f64, ry: f64) {
    // Four cubic Beziers; avoids relying on a scaled CTM after path creation.
    const K: f64 = 0.552_284_749_830_793_6;
    ctx.new_path();
    ctx.move_to(cx + rx, cy);
    ctx.curve_to(cx + rx, cy + K * ry, cx + K * rx, cy + ry, cx, cy + ry);
    ctx.curve_to(cx - K * rx, cy + ry, cx - rx, cy + K * ry, cx - rx, cy);
    ctx.curve_to(cx - rx, cy - K * ry, cx - K * rx, cy - ry, cx, cy - ry);
    ctx.curve_to(cx + K * rx, cy - ry, cx + rx, cy - K * ry, cx + rx, cy);
    ctx.close_path();
}

// ---- Primary tools -------------------------------------------------------------

fn draw_drag(ctx: &Context) {
    for (x, y) in [
        (9.0, 6.0),
        (15.0, 6.0),
        (9.0, 12.0),
        (15.0, 12.0),
        (9.0, 18.0),
        (15.0, 18.0),
    ] {
        dot(ctx, x, y, 1.25);
    }
}

fn draw_select(ctx: &Context) {
    ctx.new_path();
    ctx.move_to(5.0, 3.5);
    ctx.line_to(17.8, 12.2);
    ctx.line_to(11.9, 13.6);
    ctx.line_to(9.2, 19.2);
    ctx.close_path();
    stroke(ctx);

    ctx.move_to(12.1, 13.6);
    ctx.line_to(17.1, 18.6);
    stroke(ctx);
}

fn draw_pen(ctx: &Context) {
    ctx.new_path();
    ctx.move_to(6.4, 16.5);
    ctx.line_to(15.7, 7.2);
    ctx.curve_to(16.55, 6.35, 17.95, 6.35, 18.8, 7.2);
    ctx.curve_to(19.65, 8.05, 19.65, 9.45, 18.8, 10.3);
    ctx.line_to(9.5, 19.6);
    ctx.line_to(5.4, 20.5);
    ctx.close_path();
    stroke(ctx);

    ctx.move_to(14.7, 8.3);
    ctx.line_to(17.7, 11.3);
    stroke(ctx);

    // Fine pressure stroke: semantic cue distinguishing Pen from Marker.
    ctx.move_to(3.5, 21.0);
    ctx.curve_to(5.8, 19.8, 8.2, 19.8, 10.5, 21.0);
    stroke(ctx);
}

fn draw_marker(ctx: &Context) {
    ctx.new_path();
    ctx.move_to(8.0, 3.0);
    ctx.line_to(16.0, 3.0);
    ctx.line_to(16.0, 12.0);
    ctx.line_to(12.0, 17.0);
    ctx.line_to(8.0, 12.0);
    ctx.close_path();
    stroke(ctx);

    ctx.move_to(8.0, 8.0);
    ctx.line_to(16.0, 8.0);
    stroke(ctx);
    ctx.move_to(9.5, 14.5);
    ctx.line_to(14.5, 14.5);
    stroke(ctx);

    // Deliberate optical exception: a broad, uniform marker swatch.
    ctx.set_line_width(3.0);
    ctx.move_to(5.0, 21.0);
    ctx.line_to(19.0, 21.0);
    stroke(ctx);
    ctx.set_line_width(2.0);
}

fn draw_step_marker(ctx: &Context) {
    circle(ctx, 10.5, 11.5, 7.0);
    stroke(ctx);

    // Callout leader tail; this is what prevents a numbered-list reading.
    ctx.move_to(15.7, 16.2);
    ctx.line_to(18.5, 19.0);
    ctx.line_to(17.5, 14.7);
    stroke(ctx);

    // Vector-built numeral 1; no font dependency.
    ctx.move_to(9.2, 9.3);
    ctx.line_to(11.0, 8.2);
    ctx.line_to(11.0, 15.3);
    stroke(ctx);
    ctx.move_to(9.4, 15.3);
    ctx.line_to(12.6, 15.3);
    stroke(ctx);
}

fn draw_eraser(ctx: &Context) {
    ctx.new_path();
    ctx.move_to(4.1, 14.9);
    ctx.line_to(12.5, 6.5);
    ctx.curve_to(13.28, 5.72, 14.52, 5.72, 15.3, 6.5);
    ctx.line_to(19.5, 10.7);
    ctx.curve_to(20.28, 11.48, 20.28, 12.72, 19.5, 13.5);
    ctx.line_to(13.0, 20.0);
    ctx.line_to(8.9, 20.0);
    ctx.close_path();
    stroke(ctx);

    ctx.move_to(7.5, 11.5);
    ctx.line_to(14.4, 18.4);
    stroke(ctx);
    ctx.move_to(13.0, 20.0);
    ctx.line_to(20.0, 20.0);
    stroke(ctx);
}

fn draw_line(ctx: &Context) {
    ctx.move_to(5.0, 18.5);
    ctx.line_to(19.0, 5.5);
    stroke(ctx);
    dot(ctx, 5.0, 18.5, 1.25);
    dot(ctx, 19.0, 5.5, 1.25);
}

fn draw_arrow(ctx: &Context) {
    ctx.move_to(5.0, 5.5);
    ctx.line_to(19.0, 18.5);
    stroke(ctx);
    ctx.move_to(13.0, 18.5);
    ctx.line_to(19.0, 18.5);
    ctx.line_to(19.0, 12.5);
    stroke(ctx);
}

fn draw_shape_picker(ctx: &Context) {
    rounded_rect(ctx, 4.0, 4.0, 7.0, 7.0, 1.5);
    stroke(ctx);
    circle(ctx, 8.0, 17.0, 3.0);
    stroke(ctx);
    ctx.move_to(15.5, 11.0);
    ctx.line_to(20.0, 19.0);
    ctx.line_to(11.0, 19.0);
    ctx.close_path();
    stroke(ctx);
}

fn draw_text(ctx: &Context) {
    ctx.move_to(5.0, 5.0);
    ctx.line_to(19.0, 5.0);
    stroke(ctx);
    ctx.move_to(12.0, 5.0);
    ctx.line_to(12.0, 19.0);
    stroke(ctx);
    ctx.move_to(9.0, 19.0);
    ctx.line_to(15.0, 19.0);
    stroke(ctx);
}

fn draw_sticky_note(ctx: &Context) {
    ctx.new_path();
    ctx.move_to(6.0, 3.0);
    ctx.line_to(18.0, 3.0);
    ctx.curve_to(19.66, 3.0, 21.0, 4.34, 21.0, 6.0);
    ctx.line_to(21.0, 16.0);
    ctx.line_to(16.0, 21.0);
    ctx.line_to(6.0, 21.0);
    ctx.curve_to(4.34, 21.0, 3.0, 19.66, 3.0, 18.0);
    ctx.line_to(3.0, 6.0);
    ctx.curve_to(3.0, 4.34, 4.34, 3.0, 6.0, 3.0);
    ctx.close_path();
    stroke(ctx);

    ctx.move_to(16.0, 21.0);
    ctx.line_to(16.0, 16.0);
    ctx.line_to(21.0, 16.0);
    stroke(ctx);
    ctx.move_to(8.0, 8.0);
    ctx.line_to(15.0, 8.0);
    stroke(ctx);
    ctx.move_to(8.0, 12.0);
    ctx.line_to(13.0, 12.0);
    stroke(ctx);
}

// ---- Annotation utilities ------------------------------------------------------

fn draw_screenshot(ctx: &Context) {
    for (x1, y1, x2, y2, x3, y3) in [
        (4.0, 9.0, 4.0, 5.0, 8.0, 5.0),
        (16.0, 5.0, 20.0, 5.0, 20.0, 9.0),
        (20.0, 15.0, 20.0, 19.0, 16.0, 19.0),
        (8.0, 19.0, 4.0, 19.0, 4.0, 15.0),
    ] {
        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        ctx.line_to(x3, y3);
        stroke(ctx);
    }
    circle(ctx, 12.0, 12.0, 3.25);
    stroke(ctx);
}

fn draw_highlight(ctx: &Context) {
    circle(ctx, 12.0, 12.0, 3.25);
    stroke(ctx);
    circle(ctx, 12.0, 12.0, 7.5);
    stroke(ctx);
    dot(ctx, 12.0, 12.0, 1.0);
}

fn draw_undo(ctx: &Context) {
    ctx.move_to(8.5, 7.0);
    ctx.line_to(4.5, 11.0);
    ctx.line_to(8.5, 15.0);
    stroke(ctx);
    ctx.move_to(4.5, 11.0);
    ctx.line_to(13.0, 11.0);
    ctx.curve_to(16.31, 11.0, 19.0, 13.69, 19.0, 17.0);
    stroke(ctx);
}

fn draw_redo(ctx: &Context) {
    ctx.move_to(15.5, 7.0);
    ctx.line_to(19.5, 11.0);
    ctx.line_to(15.5, 15.0);
    stroke(ctx);
    ctx.move_to(19.5, 11.0);
    ctx.line_to(11.0, 11.0);
    ctx.curve_to(7.69, 11.0, 5.0, 13.69, 5.0, 17.0);
    stroke(ctx);
}

fn draw_clear_canvas(ctx: &Context) {
    ctx.move_to(4.0, 6.0);
    ctx.line_to(20.0, 6.0);
    stroke(ctx);
    ctx.move_to(9.0, 6.0);
    ctx.line_to(9.0, 4.0);
    ctx.line_to(15.0, 4.0);
    ctx.line_to(15.0, 6.0);
    stroke(ctx);
    ctx.move_to(6.5, 6.0);
    ctx.line_to(7.3, 20.0);
    ctx.line_to(16.7, 20.0);
    ctx.line_to(17.5, 6.0);
    stroke(ctx);
    ctx.move_to(10.0, 10.0);
    ctx.line_to(10.0, 16.0);
    stroke(ctx);
    ctx.move_to(14.0, 10.0);
    ctx.line_to(14.0, 16.0);
    stroke(ctx);
}

// ---- Toolbar chrome ------------------------------------------------------------

fn draw_overflow(ctx: &Context) {
    for x in [6.0, 12.0, 18.0] {
        dot(ctx, x, 12.0, 1.3);
    }
}

fn draw_pin(ctx: &Context) {
    ctx.new_path();
    ctx.move_to(8.0, 4.0);
    ctx.line_to(16.0, 4.0);
    ctx.line_to(14.6, 9.0);
    ctx.line_to(17.0, 11.5);
    ctx.line_to(17.0, 14.0);
    ctx.line_to(7.0, 14.0);
    ctx.line_to(7.0, 11.5);
    ctx.line_to(9.4, 9.0);
    ctx.close_path();
    stroke(ctx);
    ctx.move_to(12.0, 14.0);
    ctx.line_to(12.0, 21.0);
    stroke(ctx);
}

fn draw_unpin(ctx: &Context) {
    ctx.new_path();
    ctx.move_to(8.7, 4.0);
    ctx.line_to(16.0, 4.0);
    ctx.line_to(14.6, 9.0);
    ctx.line_to(17.0, 11.5);
    ctx.line_to(17.0, 14.0);
    ctx.line_to(14.0, 14.0);
    stroke(ctx);
    ctx.move_to(10.3, 14.0);
    ctx.line_to(7.0, 14.0);
    ctx.line_to(7.0, 11.5);
    ctx.line_to(8.5, 9.9);
    stroke(ctx);
    ctx.move_to(12.0, 14.0);
    ctx.line_to(12.0, 21.0);
    stroke(ctx);
    ctx.move_to(4.0, 4.0);
    ctx.line_to(20.0, 20.0);
    stroke(ctx);
}

fn draw_minimize(ctx: &Context) {
    ctx.move_to(5.0, 5.0);
    ctx.line_to(5.0, 8.0);
    ctx.line_to(19.0, 8.0);
    ctx.line_to(19.0, 5.0);
    stroke(ctx);
    ctx.move_to(8.5, 15.5);
    ctx.line_to(12.0, 12.0);
    ctx.line_to(15.5, 15.5);
    stroke(ctx);
}

fn draw_side_minimize(ctx: &Context) {
    ctx.move_to(5.0, 5.0);
    ctx.line_to(8.0, 5.0);
    ctx.line_to(8.0, 19.0);
    ctx.line_to(5.0, 19.0);
    stroke(ctx);
    ctx.move_to(15.5, 8.5);
    ctx.line_to(12.0, 12.0);
    ctx.line_to(15.5, 15.5);
    stroke(ctx);
}

fn draw_restore(ctx: &Context) {
    ctx.move_to(5.0, 5.0);
    ctx.line_to(5.0, 8.0);
    ctx.line_to(19.0, 8.0);
    ctx.line_to(19.0, 5.0);
    stroke(ctx);
    ctx.move_to(8.5, 11.5);
    ctx.line_to(12.0, 15.0);
    ctx.line_to(15.5, 11.5);
    stroke(ctx);
}

#[allow(dead_code)] // complete family entry point; no close action today
fn draw_close(ctx: &Context) {
    ctx.move_to(6.0, 6.0);
    ctx.line_to(18.0, 18.0);
    stroke(ctx);
    ctx.move_to(18.0, 6.0);
    ctx.line_to(6.0, 18.0);
    stroke(ctx);
}

// ---- Shape palette -------------------------------------------------------------

fn draw_rectangle(ctx: &Context) {
    rounded_rect(ctx, 4.0, 6.0, 16.0, 12.0, 2.0);
    stroke(ctx);
}

fn draw_ellipse(ctx: &Context) {
    ellipse(ctx, 12.0, 12.0, 8.0, 6.0);
    stroke(ctx);
}

fn draw_blur(ctx: &Context) {
    rounded_rect(ctx, 4.0, 4.0, 16.0, 16.0, 3.0);
    stroke(ctx);
    for (x, y, r) in [
        (8.0, 8.0, 1.0),
        (13.0, 8.0, 1.0),
        (16.5, 11.5, 1.0),
        (11.0, 12.0, 1.4),
        (7.5, 15.5, 1.0),
        (14.0, 16.0, 1.0),
    ] {
        dot(ctx, x, y, r);
    }
}

fn draw_triangle(ctx: &Context) {
    ctx.move_to(12.0, 4.0);
    ctx.line_to(20.0, 19.0);
    ctx.line_to(4.0, 19.0);
    ctx.close_path();
    stroke(ctx);
}

fn draw_parallelogram(ctx: &Context) {
    ctx.move_to(8.0, 5.0);
    ctx.line_to(20.0, 5.0);
    ctx.line_to(16.0, 19.0);
    ctx.line_to(4.0, 19.0);
    ctx.close_path();
    stroke(ctx);
}

fn draw_rhombus(ctx: &Context) {
    ctx.move_to(12.0, 3.0);
    ctx.line_to(21.0, 12.0);
    ctx.line_to(12.0, 21.0);
    ctx.line_to(3.0, 12.0);
    ctx.close_path();
    stroke(ctx);
}

fn draw_regular_polygon(ctx: &Context) {
    ctx.move_to(12.0, 3.5);
    ctx.line_to(20.2, 9.5);
    ctx.line_to(17.1, 19.1);
    ctx.line_to(6.9, 19.1);
    ctx.line_to(3.8, 9.5);
    ctx.close_path();
    stroke(ctx);
}

fn draw_freeform_polygon(ctx: &Context) {
    let points = [
        (4.5, 15.5),
        (7.5, 5.5),
        (15.5, 7.5),
        (19.5, 18.5),
        (9.5, 20.0),
    ];
    ctx.move_to(points[0].0, points[0].1);
    for &(x, y) in &points[1..] {
        ctx.line_to(x, y);
    }
    ctx.close_path();
    stroke(ctx);
    for (x, y) in points {
        dot(ctx, x, y, 1.15);
    }
}

#[allow(dead_code)] // complete family entry point; not wired to an option yet
fn draw_fill(ctx: &Context) {
    ctx.move_to(5.0, 12.0);
    ctx.line_to(12.0, 5.0);
    ctx.line_to(19.0, 12.0);
    ctx.line_to(12.0, 19.0);
    ctx.close_path();
    stroke(ctx);
    ctx.move_to(12.0, 5.0);
    ctx.line_to(16.0, 9.0);
    stroke(ctx);

    ctx.new_path();
    ctx.move_to(20.0, 15.0);
    ctx.curve_to(21.2, 16.5, 22.0, 17.7, 22.0, 18.8);
    ctx.curve_to(22.0, 19.9, 21.1, 20.8, 20.0, 20.8);
    ctx.curve_to(18.9, 20.8, 18.0, 19.9, 18.0, 18.8);
    ctx.curve_to(18.0, 17.7, 18.8, 16.5, 20.0, 15.0);
    ctx.close_path();
    stroke(ctx);
}

#[allow(dead_code)] // complete family entry point; not wired to an option yet
fn draw_highlight_ring(ctx: &Context) {
    circle(ctx, 12.0, 12.0, 6.0);
    stroke(ctx);
    for (x1, y1, x2, y2) in [
        (12.0, 3.0, 12.0, 5.0),
        (12.0, 19.0, 12.0, 21.0),
        (3.0, 12.0, 5.0, 12.0),
        (19.0, 12.0, 21.0, 12.0),
    ] {
        ctx.move_to(x1, y1);
        ctx.line_to(x2, y2);
        stroke(ctx);
    }
}

// ---- Public render entry points ------------------------------------------------

macro_rules! renderers {
    ($(($public:ident, $draw:ident)),+ $(,)?) => {
        $(
            #[inline]
            #[allow(dead_code)]
            pub fn $public(ctx: &Context, x: f64, y: f64, size: f64) {
                render_icon(ctx, x, y, size, $draw);
            }
        )+
    };
}

renderers!(
    (render_drag, draw_drag),
    (render_select, draw_select),
    (render_pen, draw_pen),
    (render_marker, draw_marker),
    (render_step_marker, draw_step_marker),
    (render_eraser, draw_eraser),
    (render_line, draw_line),
    (render_arrow, draw_arrow),
    (render_shape_picker, draw_shape_picker),
    (render_text, draw_text),
    (render_note, draw_sticky_note),
    (render_screenshot, draw_screenshot),
    (render_highlight, draw_highlight),
    (render_undo, draw_undo),
    (render_redo, draw_redo),
    (render_clear_canvas, draw_clear_canvas),
    (render_more, draw_overflow),
    (render_pin, draw_pin),
    (render_unpin, draw_unpin),
    (render_minimize, draw_minimize),
    (render_side_minimize, draw_side_minimize),
    (render_restore, draw_restore),
    (render_close, draw_close),
    (render_rect, draw_rectangle),
    (render_circle, draw_ellipse),
    (render_blur, draw_blur),
    (render_triangle, draw_triangle),
    (render_parallelogram, draw_parallelogram),
    (render_rhombus, draw_rhombus),
    (render_polygon, draw_regular_polygon),
    (render_freeform_polygon, draw_freeform_polygon),
    (render_fill, draw_fill),
    (render_highlight_ring, draw_highlight_ring),
);

#[cfg(test)]
mod tests {
    use super::*;
    use cairo::{Format, ImageSurface};

    type IconRender = fn(&Context, f64, f64, f64);

    const SIZES: [i32; 5] = [18, 20, 22, 24, 28];
    const ICONS: [(&str, IconRender); 33] = [
        ("drag", render_drag),
        ("select", render_select),
        ("pen", render_pen),
        ("marker", render_marker),
        ("step_marker", render_step_marker),
        ("eraser", render_eraser),
        ("line", render_line),
        ("arrow", render_arrow),
        ("shape_picker", render_shape_picker),
        ("text", render_text),
        ("sticky_note", render_note),
        ("screenshot", render_screenshot),
        ("highlight", render_highlight),
        ("undo", render_undo),
        ("redo", render_redo),
        ("clear_canvas", render_clear_canvas),
        ("overflow", render_more),
        ("pin", render_pin),
        ("unpin", render_unpin),
        ("minimize", render_minimize),
        ("side_minimize", render_side_minimize),
        ("restore", render_restore),
        ("close", render_close),
        ("rectangle", render_rect),
        ("ellipse", render_circle),
        ("blur", render_blur),
        ("triangle", render_triangle),
        ("parallelogram", render_parallelogram),
        ("rhombus", render_rhombus),
        ("regular_polygon", render_polygon),
        ("freeform_polygon", render_freeform_polygon),
        ("fill", render_fill),
        ("highlight_ring", render_highlight_ring),
    ];

    #[test]
    fn all_icons_render_non_empty_at_required_sizes() {
        for (name, draw) in ICONS {
            for size in SIZES {
                let surface = ImageSurface::create(Format::ARgb32, size, size).expect("surface");
                let ctx = Context::new(&surface).expect("context");
                ctx.set_source_rgba(1.0, 1.0, 1.0, 1.0);
                draw(&ctx, 0.0, 0.0, f64::from(size));
                surface.flush();
                let mut has_alpha = false;
                surface
                    .with_data(|pixels| {
                        has_alpha = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
                    })
                    .expect("surface data");
                assert!(has_alpha, "{name} rendered empty at {size}px");
            }
        }
    }
}
