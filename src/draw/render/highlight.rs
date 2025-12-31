use crate::draw::Color;

/// Renders a circular click highlight with configurable fill/outline colors.
#[allow(clippy::too_many_arguments)]
pub fn render_click_highlight(
    ctx: &cairo::Context,
    center_x: f64,
    center_y: f64,
    radius: f64,
    outline_thickness: f64,
    fill_color: Color,
    outline_color: Color,
    opacity: f64,
) {
    if opacity <= 0.0 {
        return;
    }

    let alpha = opacity.clamp(0.0, 1.0);
    let radius = radius.max(1.0);
    let _ = ctx.save();

    if fill_color.a > 0.0 {
        ctx.set_source_rgba(
            fill_color.r,
            fill_color.g,
            fill_color.b,
            fill_color.a * alpha,
        );
        ctx.arc(center_x, center_y, radius, 0.0, std::f64::consts::PI * 2.0);
        let _ = ctx.fill();
    }

    if outline_color.a > 0.0 && outline_thickness > 0.0 {
        ctx.set_source_rgba(
            outline_color.r,
            outline_color.g,
            outline_color.b,
            outline_color.a * alpha,
        );
        ctx.set_line_width(outline_thickness);
        ctx.arc(center_x, center_y, radius, 0.0, std::f64::consts::PI * 2.0);
        let _ = ctx.stroke();
    }

    let _ = ctx.restore();
}
