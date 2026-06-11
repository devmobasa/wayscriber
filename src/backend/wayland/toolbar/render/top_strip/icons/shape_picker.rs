use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::bindings::tool_tooltip_label;
use crate::ui::toolbar::model;

use super::super::super::widgets::*;
use super::TopStripLayout;
use super::tool_row::draw_semantic_tool_icon;

pub(super) fn draw_shape_picker_row(
    layout: &mut TopStripLayout,
    handle_w: f64,
    y: f64,
    btn_size: f64,
    icon_size: f64,
    is_simple: bool,
) {
    let mut shape_y = y + btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
    if !model::top_shape_picker_visible(layout.snapshot) {
        return;
    }

    for row in model::visible_shape_picker_rows(layout.snapshot, is_simple) {
        draw_picker_row(layout, handle_w, shape_y, btn_size, icon_size, &row);
        shape_y += btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
    }
}

fn draw_picker_row(
    layout: &mut TopStripLayout,
    handle_w: f64,
    shape_y: f64,
    btn_size: f64,
    icon_size: f64,
    tools: &[crate::input::Tool],
) {
    let mut shape_x = ToolbarLayoutSpec::TOP_START_X + handle_w + layout.gap;
    for tool in tools {
        if !model::tool_visible(layout.snapshot, *tool) {
            continue;
        }
        let is_active =
            layout.snapshot.active_tool == *tool || layout.snapshot.tool_override == Some(*tool);
        let is_hover = layout
            .hover
            .map(|(hx, hy)| point_in_rect(hx, hy, shape_x, shape_y, btn_size, btn_size))
            .unwrap_or(false);
        draw_button(
            layout.ctx, shape_x, shape_y, btn_size, btn_size, is_active, is_hover,
        );
        set_icon_color(layout.ctx, is_hover);
        let icon_x = shape_x + (btn_size - icon_size) / 2.0;
        let icon_y = shape_y + (btn_size - icon_size) / 2.0;
        draw_semantic_tool_icon(
            layout.ctx,
            model::semantic_icon_for_tool(*tool),
            icon_x,
            icon_y,
            icon_size,
        );
        let tooltip = layout.tool_tooltip(*tool, tool_tooltip_label(*tool));
        layout.hits.push(HitRegion {
            rect: (shape_x, shape_y, btn_size, btn_size),
            event: ToolbarEvent::SelectTool(*tool),
            kind: HitKind::Click,
            tooltip: Some(tooltip),
        });
        shape_x += btn_size + layout.gap;
    }
}
