use cairo::Context;
use std::f64::consts::PI;

/// Draw a cursor/select icon (arrow pointer)
pub fn draw_icon_select(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.06).max(1.2);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Compact cursor arrow with a short tail.
    ctx.move_to(x + s * 0.2, y + s * 0.12);
    ctx.line_to(x + s * 0.2, y + s * 0.78);
    ctx.line_to(x + s * 0.36, y + s * 0.62);
    ctx.line_to(x + s * 0.5, y + s * 0.88);
    ctx.line_to(x + s * 0.62, y + s * 0.82);
    ctx.line_to(x + s * 0.46, y + s * 0.56);
    ctx.line_to(x + s * 0.78, y + s * 0.46);
    ctx.close_path();

    let _ = ctx.fill_preserve();
    let _ = ctx.stroke();
}

/// Draw a pen/freehand icon (nib with a short stroke)
pub fn draw_icon_pen(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.4);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Fountain pen nib (diamond) with a short drawing stroke.
    let cx = x + s * 0.6;
    let cy = y + s * 0.38;
    let nib_w = s * 0.32;
    let nib_h = s * 0.36;

    ctx.move_to(cx, cy - nib_h * 0.5);
    ctx.line_to(cx + nib_w * 0.5, cy);
    ctx.line_to(cx, cy + nib_h * 0.5);
    ctx.line_to(cx - nib_w * 0.5, cy);
    ctx.close_path();
    let _ = ctx.stroke();

    ctx.move_to(cx, cy + nib_h * 0.05);
    ctx.line_to(cx, cy + nib_h * 0.5);
    let _ = ctx.stroke();

    ctx.set_line_width((s * 0.1).max(1.4));
    ctx.move_to(x + s * 0.16, y + s * 0.78);
    ctx.curve_to(
        x + s * 0.3,
        y + s * 0.68,
        x + s * 0.42,
        y + s * 0.86,
        x + s * 0.62,
        y + s * 0.74,
    );
    let _ = ctx.stroke();
}

/// Draw a line tool icon
pub fn draw_icon_line(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x + s * 0.2, y + s * 0.8);
    ctx.line_to(x + s * 0.8, y + s * 0.2);
    let _ = ctx.stroke();
}

/// Draw a rectangle tool icon
pub fn draw_icon_rect(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);

    let margin = s * 0.2;
    ctx.rectangle(x + margin, y + margin, s - margin * 2.0, s - margin * 2.0);
    let _ = ctx.stroke();
}

/// Draw a circle/ellipse tool icon
pub fn draw_icon_circle(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;
    let r = s * 0.35;
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw an arrow tool icon
pub fn draw_icon_arrow(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Arrow line
    ctx.move_to(x + s * 0.2, y + s * 0.8);
    ctx.line_to(x + s * 0.8, y + s * 0.2);
    let _ = ctx.stroke();

    // Arrow head
    ctx.move_to(x + s * 0.8, y + s * 0.2);
    ctx.line_to(x + s * 0.55, y + s * 0.25);
    let _ = ctx.stroke();
    ctx.move_to(x + s * 0.8, y + s * 0.2);
    ctx.line_to(x + s * 0.75, y + s * 0.45);
    let _ = ctx.stroke();
}

/// Draw an eraser tool icon
#[allow(dead_code)]
pub fn draw_icon_eraser(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.4);
    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Slanted eraser block with a band near the tip.
    let body_w = s * 0.72;
    let body_h = s * 0.34;
    let angle = -PI * 0.22;
    let cx = x + s * 0.55;
    let cy = y + s * 0.55;

    let _ = ctx.save();
    ctx.translate(cx, cy);
    ctx.rotate(angle);
    ctx.rectangle(-body_w / 2.0, -body_h / 2.0, body_w, body_h);
    let _ = ctx.stroke();

    let band_x = -body_w * 0.18;
    ctx.move_to(band_x, -body_h / 2.0);
    ctx.line_to(band_x, body_h / 2.0);
    let _ = ctx.stroke();
    let _ = ctx.restore();
}

/// Draw a text tool icon (letter T)
pub fn draw_icon_text(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.12).max(2.0);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);

    // Top bar of T
    ctx.move_to(x + s * 0.2, y + s * 0.25);
    ctx.line_to(x + s * 0.8, y + s * 0.25);
    let _ = ctx.stroke();

    // Vertical bar of T
    ctx.move_to(x + s * 0.5, y + s * 0.25);
    ctx.line_to(x + s * 0.5, y + s * 0.8);
    let _ = ctx.stroke();
}

/// Draw a sticky note icon (square with folded corner)
pub fn draw_icon_note(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.4);
    let margin = s * 0.18;
    let fold = s * 0.22;

    ctx.set_line_width(stroke);
    ctx.set_line_join(cairo::LineJoin::Round);
    ctx.set_line_cap(cairo::LineCap::Round);

    ctx.move_to(x + margin, y + margin);
    ctx.line_to(x + s - margin - fold, y + margin);
    ctx.line_to(x + s - margin, y + margin + fold);
    ctx.line_to(x + s - margin, y + s - margin);
    ctx.line_to(x + margin, y + s - margin);
    ctx.close_path();
    let _ = ctx.stroke();

    ctx.move_to(x + s - margin - fold, y + margin);
    ctx.line_to(x + s - margin - fold, y + margin + fold);
    ctx.line_to(x + s - margin, y + margin + fold);
    let _ = ctx.stroke();
}

/// Draw a highlighter tool icon (cursor with click ripple effect)
#[allow(dead_code)]
pub fn draw_icon_highlight(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.5);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Pointer/cursor arrow
    ctx.move_to(x + s * 0.2, y + s * 0.15);
    ctx.line_to(x + s * 0.2, y + s * 0.65);
    ctx.line_to(x + s * 0.35, y + s * 0.52);
    ctx.line_to(x + s * 0.5, y + s * 0.75);
    ctx.line_to(x + s * 0.58, y + s * 0.7);
    ctx.line_to(x + s * 0.43, y + s * 0.47);
    ctx.line_to(x + s * 0.58, y + s * 0.4);
    ctx.close_path();
    let _ = ctx.fill();

    // Ripple circles around click point
    ctx.set_line_width((s * 0.08).max(1.0));
    // Inner ripple
    ctx.arc(x + s * 0.7, y + s * 0.55, s * 0.12, 0.0, PI * 2.0);
    let _ = ctx.stroke();
    // Outer ripple
    ctx.arc(x + s * 0.7, y + s * 0.55, s * 0.22, 0.0, PI * 2.0);
    let _ = ctx.stroke();
}

/// Draw a marker/highlighter icon (tilted marker with translucent swatch)
pub fn draw_icon_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.08).max(1.4);
    let swatch_color = (1.0, 0.92, 0.35, 0.4);

    // Underlying swatch to imply transparent ink
    ctx.set_source_rgba(
        swatch_color.0,
        swatch_color.1,
        swatch_color.2,
        swatch_color.3,
    );
    let swatch_h = s * 0.28;
    ctx.rectangle(x + s * 0.08, y + s * 0.65, s * 0.84, swatch_h);
    let _ = ctx.fill();

    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    // Marker body (tilted)
    ctx.save().ok();
    ctx.translate(x + s * 0.55, y + s * 0.35);
    ctx.rotate(-PI * 0.18);

    // Body outline
    ctx.set_source_rgba(0.95, 0.95, 0.98, 0.95);
    ctx.rectangle(-s * 0.24, -s * 0.08, s * 0.38, s * 0.26);
    let _ = ctx.fill_preserve();
    ctx.set_source_rgba(0.25, 0.28, 0.35, 0.9);
    let _ = ctx.stroke();

    // Chisel tip with ink color
    ctx.set_source_rgba(swatch_color.0, swatch_color.1, swatch_color.2, 0.85);
    ctx.move_to(-s * 0.24, -s * 0.08);
    ctx.line_to(-s * 0.32, 0.0);
    ctx.line_to(-s * 0.24, s * 0.18);
    ctx.close_path();
    let _ = ctx.fill();
    ctx.restore().ok();
}

/// Draw a step marker icon (numbered circle)
pub fn draw_icon_step_marker(ctx: &Context, x: f64, y: f64, size: f64) {
    let s = size;
    let stroke = (s * 0.1).max(1.6);
    ctx.set_line_width(stroke);
    ctx.set_line_cap(cairo::LineCap::Round);
    ctx.set_line_join(cairo::LineJoin::Round);

    let cx = x + s * 0.5;
    let cy = y + s * 0.5;
    let r = s * 0.32;
    // Offset bubble to hint at a sequence
    ctx.arc(cx - r * 0.45, cy - r * 0.45, r * 0.55, 0.0, PI * 2.0);
    let _ = ctx.stroke();
    ctx.arc(cx, cy, r, 0.0, PI * 2.0);
    let _ = ctx.stroke();

    // Stylized "1" glyph
    let one_h = r * 1.15;
    let one_w = r * 0.35;
    ctx.set_line_width((s * 0.12).max(1.8));
    ctx.move_to(cx - one_w * 0.3, cy - one_h * 0.45);
    ctx.line_to(cx, cy - one_h * 0.6);
    ctx.line_to(cx + one_w * 0.25, cy - one_h * 0.35);
    let _ = ctx.stroke();
    ctx.move_to(cx, cy - one_h * 0.45);
    ctx.line_to(cx, cy + one_h * 0.5);
    let _ = ctx.stroke();
    ctx.move_to(cx - one_w, cy + one_h * 0.5);
    ctx.line_to(cx + one_w, cy + one_h * 0.5);
    let _ = ctx.stroke();
}
