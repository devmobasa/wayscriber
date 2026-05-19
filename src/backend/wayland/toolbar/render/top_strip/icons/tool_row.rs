use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::bindings::tool_tooltip_label;
use crate::ui::toolbar::model::{self, SemanticToolIcon};

use super::super::super::widgets::*;
use super::TopStripLayout;

pub(super) struct ToolRowResult {
    pub(super) next_x: f64,
    pub(super) fill_anchor: Option<(f64, f64)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_tool_row(
    layout: &mut TopStripLayout,
    mut x: f64,
    y: f64,
    btn_size: f64,
    icon_size: f64,
    gap: f64,
    is_simple: bool,
    current_shape_tool: Option<Tool>,
    shape_icon_tool: Tool,
) -> ToolRowResult {
    let snapshot = layout.snapshot;

    let mut fill_anchor: Option<(f64, f64)> = None;
    let mut rect_x = None;
    let mut circle_end_x = None;
    for tool in model::top_tool_buttons(is_simple) {
        if model::is_fill_tool(*tool) && rect_x.is_none() {
            rect_x = Some(x);
        }
        if model::is_fill_tool(*tool) {
            circle_end_x = Some(x + btn_size);
        }

        let is_active = snapshot.active_tool == *tool || snapshot.tool_override == Some(*tool);
        let is_hover = layout
            .hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(layout.ctx, x, y, btn_size, btn_size, is_active, is_hover);

        set_icon_color(layout.ctx, is_hover);
        let icon_x = x + (btn_size - icon_size) / 2.0;
        let icon_y = y + (btn_size - icon_size) / 2.0;
        draw_semantic_tool_icon(
            layout.ctx,
            model::semantic_icon_for_tool(*tool),
            icon_x,
            icon_y,
            icon_size,
        );

        let tooltip = layout.tool_tooltip(*tool, tool_tooltip_label(*tool));
        layout.hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(tooltip),
        });
        x += btn_size + gap;
    }

    if is_simple {
        let shapes_active = snapshot.shape_picker_open || current_shape_tool.is_some();
        let shapes_hover = layout
            .hover
            .map(|(hx, hy)| point_in_rect(hx, hy, x, y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(
            layout.ctx,
            x,
            y,
            btn_size,
            btn_size,
            shapes_active,
            shapes_hover,
        );
        set_icon_color(layout.ctx, shapes_hover);
        let icon_x = x + (btn_size - icon_size) / 2.0;
        let icon_y = y + (btn_size - icon_size) / 2.0;
        draw_semantic_tool_icon(
            layout.ctx,
            model::semantic_icon_for_tool(shape_icon_tool),
            icon_x,
            icon_y,
            icon_size,
        );
        layout.hits.push(HitRegion {
            rect: (x, y, btn_size, btn_size),
            event: ToolbarEvent::ToggleShapePicker(!snapshot.shape_picker_open),
            kind: HitKind::Click,
            tooltip: Some("Shapes".to_string()),
        });
        fill_anchor = Some((x, btn_size));
        x += btn_size + gap;
    } else if let (Some(rect_x), Some(circle_end_x)) = (rect_x, circle_end_x) {
        fill_anchor = Some((rect_x, circle_end_x - rect_x));
    }

    ToolRowResult {
        next_x: x,
        fill_anchor,
    }
}

pub(crate) fn draw_semantic_tool_icon(
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
