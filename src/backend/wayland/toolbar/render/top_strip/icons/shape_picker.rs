use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::input::Tool;
use crate::toolbar_icons;
use crate::ui::toolbar::ToolbarEvent;
use crate::ui::toolbar::bindings::tool_tooltip_label;

use super::super::super::widgets::*;
use super::TopStripLayout;

type IconFn = fn(&cairo::Context, f64, f64, f64);

pub(super) fn draw_shape_picker_row(
    layout: &mut TopStripLayout,
    handle_w: f64,
    y: f64,
    btn_size: f64,
    icon_size: f64,
) {
    let shape_y = y + btn_size + ToolbarLayoutSpec::TOP_SHAPE_ROW_GAP;
    let mut shape_x = ToolbarLayoutSpec::TOP_START_X + handle_w + layout.gap;
    let shapes: &[(Tool, IconFn)] = &[
        (Tool::Line, toolbar_icons::draw_icon_line),
        (Tool::Rect, toolbar_icons::draw_icon_rect),
        (Tool::Ellipse, toolbar_icons::draw_icon_circle),
        (Tool::Arrow, toolbar_icons::draw_icon_arrow),
    ];
    for (tool, icon_fn) in shapes {
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
        icon_fn(layout.ctx, icon_x, icon_y, icon_size);
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
