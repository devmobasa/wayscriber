use crate::draw::Color;
use crate::input::Tool;
use crate::toolbar_icons;

pub(super) fn draw_tool_preview(
    ctx: &cairo::Context,
    tool: Tool,
    color: Color,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) {
    let icon_size = 18.0;
    let pad = 6.0;
    let bubble = icon_size + pad * 2.0;
    let mut bx = x + 16.0;
    let mut by = y + 16.0;
    let max_x = (w - bubble - 4.0).max(4.0);
    let max_y = (h - bubble - 4.0).max(4.0);
    if bx < 4.0 {
        bx = 4.0;
    } else if bx > max_x {
        bx = max_x;
    }
    if by < 4.0 {
        by = 4.0;
    } else if by > max_y {
        by = max_y;
    }

    let cx = bx + bubble / 2.0;
    let cy = by + bubble / 2.0;
    let radius = bubble / 2.0;
    let _ = ctx.save();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    ctx.arc(cx + 1.0, cy + 1.5, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();
    ctx.set_source_rgba(0.08, 0.08, 0.1, 0.6);
    ctx.arc(cx, cy, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    let (r, g, b, a) = match tool {
        Tool::Eraser | Tool::Select => (0.95, 0.95, 0.98, 0.95),
        _ => (color.r, color.g, color.b, 0.95),
    };
    ctx.set_source_rgba(r, g, b, a);
    let icon_x = bx + pad;
    let icon_y = by + pad;
    match tool {
        Tool::Select => toolbar_icons::draw_icon_select(ctx, icon_x, icon_y, icon_size),
        Tool::Pen => toolbar_icons::draw_icon_pen(ctx, icon_x, icon_y, icon_size),
        Tool::Line => toolbar_icons::draw_icon_line(ctx, icon_x, icon_y, icon_size),
        Tool::Rect => toolbar_icons::draw_icon_rect(ctx, icon_x, icon_y, icon_size),
        Tool::Ellipse => toolbar_icons::draw_icon_circle(ctx, icon_x, icon_y, icon_size),
        Tool::Arrow => toolbar_icons::draw_icon_arrow(ctx, icon_x, icon_y, icon_size),
        Tool::Marker => toolbar_icons::draw_icon_marker(ctx, icon_x, icon_y, icon_size),
        Tool::Highlight => toolbar_icons::draw_icon_highlight(ctx, icon_x, icon_y, icon_size),
        Tool::Eraser => toolbar_icons::draw_icon_eraser(ctx, icon_x, icon_y, icon_size),
    }
    let _ = ctx.restore();
}
