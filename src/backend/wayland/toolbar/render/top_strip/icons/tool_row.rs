use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::bindings::tool_tooltip_label;

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

    type IconFn = fn(&cairo::Context, f64, f64, f64);

    let tool_buttons: &[(Tool, IconFn)] = if is_simple {
        &[
            (Tool::Select, toolbar_icons::draw_icon_select as IconFn),
            (Tool::Pen, toolbar_icons::draw_icon_pen as IconFn),
            (Tool::Marker, toolbar_icons::draw_icon_marker as IconFn),
            (
                Tool::StepMarker,
                toolbar_icons::draw_icon_step_marker as IconFn,
            ),
            (Tool::Eraser, toolbar_icons::draw_icon_eraser as IconFn),
        ]
    } else {
        &[
            (Tool::Select, toolbar_icons::draw_icon_select as IconFn),
            (Tool::Pen, toolbar_icons::draw_icon_pen as IconFn),
            (Tool::Marker, toolbar_icons::draw_icon_marker as IconFn),
            (
                Tool::StepMarker,
                toolbar_icons::draw_icon_step_marker as IconFn,
            ),
            (Tool::Eraser, toolbar_icons::draw_icon_eraser as IconFn),
            (Tool::Line, toolbar_icons::draw_icon_line as IconFn),
            (Tool::Rect, toolbar_icons::draw_icon_rect as IconFn),
            (Tool::Ellipse, toolbar_icons::draw_icon_circle as IconFn),
            (Tool::Arrow, toolbar_icons::draw_icon_arrow as IconFn),
        ]
    };

    let mut fill_anchor: Option<(f64, f64)> = None;
    let mut rect_x = None;
    let mut circle_end_x = None;
    for (tool, icon_fn) in tool_buttons {
        if *tool == Tool::Rect {
            rect_x = Some(x);
        }
        if *tool == Tool::Ellipse {
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
        icon_fn(layout.ctx, icon_x, icon_y, icon_size);

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
        match shape_icon_tool {
            Tool::Line => toolbar_icons::draw_icon_line(layout.ctx, icon_x, icon_y, icon_size),
            Tool::Rect => toolbar_icons::draw_icon_rect(layout.ctx, icon_x, icon_y, icon_size),
            Tool::Ellipse => toolbar_icons::draw_icon_circle(layout.ctx, icon_x, icon_y, icon_size),
            Tool::Arrow => toolbar_icons::draw_icon_arrow(layout.ctx, icon_x, icon_y, icon_size),
            _ => toolbar_icons::draw_icon_rect(layout.ctx, icon_x, icon_y, icon_size),
        }
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
