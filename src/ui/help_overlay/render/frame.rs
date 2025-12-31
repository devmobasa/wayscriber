use crate::config::HelpOverlayStyle;

use super::palette::RenderPalette;
use crate::ui::primitives::draw_rounded_rect;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_overlay_frame(
    ctx: &cairo::Context,
    style: &HelpOverlayStyle,
    palette: &RenderPalette,
    screen_width: u32,
    screen_height: u32,
    box_x: f64,
    box_y: f64,
    box_width: f64,
    box_height: f64,
) {
    let corner_radius = 16.0;

    // Dim background behind overlay
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.55);
    ctx.rectangle(0.0, 0.0, screen_width as f64, screen_height as f64);
    let _ = ctx.fill();

    // Drop shadow (layered for softer effect)
    let shadow_offset = 12.0;
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    draw_rounded_rect(
        ctx,
        box_x + shadow_offset + 4.0,
        box_y + shadow_offset + 4.0,
        box_width,
        box_height,
        corner_radius,
    );
    let _ = ctx.fill();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    draw_rounded_rect(
        ctx,
        box_x + shadow_offset,
        box_y + shadow_offset,
        box_width,
        box_height,
        corner_radius,
    );
    let _ = ctx.fill();

    // Background gradient
    let gradient = cairo::LinearGradient::new(box_x, box_y, box_x, box_y + box_height);
    gradient.add_color_stop_rgba(
        0.0,
        palette.bg_top[0],
        palette.bg_top[1],
        palette.bg_top[2],
        palette.bg_top[3],
    );
    gradient.add_color_stop_rgba(
        1.0,
        palette.bg_bottom[0],
        palette.bg_bottom[1],
        palette.bg_bottom[2],
        palette.bg_bottom[3],
    );
    let _ = ctx.set_source(&gradient);
    draw_rounded_rect(ctx, box_x, box_y, box_width, box_height, corner_radius);
    let _ = ctx.fill();

    // Border
    let [br, bg, bb, ba] = style.border_color;
    ctx.set_source_rgba(br, bg, bb, ba);
    ctx.set_line_width(style.border_width);
    draw_rounded_rect(ctx, box_x, box_y, box_width, box_height, corner_radius);
    let _ = ctx.stroke();
}
