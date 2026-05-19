use crate::draw::Color;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::model::{self, SemanticToolIcon};

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
    draw_semantic_tool_icon(
        ctx,
        model::semantic_icon_for_tool(tool),
        icon_x,
        icon_y,
        icon_size,
    );
    let _ = ctx.restore();
}

fn draw_semantic_tool_icon(
    ctx: &cairo::Context,
    icon: SemanticToolIcon,
    x: f64,
    y: f64,
    size: f64,
) {
    match icon {
        SemanticToolIcon::Select => toolbar_icons::draw_icon_select(ctx, x, y, size),
        SemanticToolIcon::Pen => toolbar_icons::draw_icon_pen(ctx, x, y, size),
        SemanticToolIcon::Line => toolbar_icons::draw_icon_line(ctx, x, y, size),
        SemanticToolIcon::Rect => toolbar_icons::draw_icon_rect(ctx, x, y, size),
        SemanticToolIcon::Circle => toolbar_icons::draw_icon_circle(ctx, x, y, size),
        SemanticToolIcon::Arrow => toolbar_icons::draw_icon_arrow(ctx, x, y, size),
        SemanticToolIcon::Blur => toolbar_icons::draw_icon_blur(ctx, x, y, size),
        SemanticToolIcon::Marker => toolbar_icons::draw_icon_marker(ctx, x, y, size),
        SemanticToolIcon::Highlight => toolbar_icons::draw_icon_highlight(ctx, x, y, size),
        SemanticToolIcon::StepMarker => toolbar_icons::draw_icon_step_marker(ctx, x, y, size),
        SemanticToolIcon::Eraser => toolbar_icons::draw_icon_eraser(ctx, x, y, size),
    }
}

pub(super) fn draw_stylus_hover_cursor(
    ctx: &cairo::Context,
    tool: Tool,
    color: Color,
    x: f64,
    y: f64,
) {
    let (r, g, b, radius) = match tool {
        Tool::Eraser | Tool::Select => (0.96, 0.96, 0.98, 4.0),
        _ => (color.r, color.g, color.b, 3.5),
    };

    let _ = ctx.save();
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    ctx.arc(
        x + 1.0,
        y + 1.0,
        radius + 2.0,
        0.0,
        std::f64::consts::PI * 2.0,
    );
    let _ = ctx.fill();

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    ctx.arc(x, y, radius + 1.4, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(r, g, b, 0.95);
    ctx.arc(x, y, radius, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.fill();

    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.72);
    ctx.set_line_width(1.0);
    ctx.arc(x, y, radius + 1.4, 0.0, std::f64::consts::PI * 2.0);
    let _ = ctx.stroke();
    let _ = ctx.restore();
}
