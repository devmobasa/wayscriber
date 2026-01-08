mod shape_picker;
mod tool_row;
mod utility_row;

use super::{ICON_TOGGLE_FONT_SIZE, TOP_LABEL_FONT_SIZE, TopStripLayout};
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::config::{Action, action_label, action_short_label};
use crate::input::Tool;
use crate::ui::toolbar::ToolbarEvent;

use super::super::widgets::*;
use shape_picker::draw_shape_picker_row;
use tool_row::draw_tool_row;
use utility_row::draw_utility_row;

pub(super) fn draw_icon_strip(
    layout: &mut TopStripLayout,
    mut x: f64,
    handle_w: f64,
    is_simple: bool,
    current_shape_tool: Option<Tool>,
    shape_icon_tool: Tool,
    fill_tool_active: bool,
) {
    let ctx = layout.ctx;
    let snapshot = layout.snapshot;
    let hover = layout.hover;
    let gap = layout.gap;

    let (btn_size, _) = layout.spec.top_button_size();
    let y = layout.spec.top_button_y(layout.height);
    let icon_size = ToolbarLayoutSpec::TOP_ICON_SIZE;
    let fill_h = ToolbarLayoutSpec::TOP_ICON_FILL_HEIGHT;

    let tool_row = draw_tool_row(
        layout,
        x,
        y,
        btn_size,
        icon_size,
        gap,
        is_simple,
        current_shape_tool,
        shape_icon_tool,
    );
    x = tool_row.next_x;

    if fill_tool_active
        && !(is_simple && snapshot.shape_picker_open)
        && let Some((fill_x, fill_w)) = tool_row.fill_anchor
    {
        let fill_y = y + btn_size + ToolbarLayoutSpec::TOP_ICON_FILL_OFFSET;
        let fill_hover = hover
            .map(|(hx, hy)| point_in_rect(hx, hy, fill_x, fill_y, fill_w, fill_h))
            .unwrap_or(false);
        draw_mini_checkbox(
            ctx,
            fill_x,
            fill_y,
            fill_w,
            fill_h,
            snapshot.fill_enabled,
            fill_hover,
            action_short_label(Action::ToggleFill),
        );
        layout.hits.push(HitRegion {
            rect: (fill_x, fill_y, fill_w, fill_h),
            event: ToolbarEvent::ToggleFill(!snapshot.fill_enabled),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                action_label(Action::ToggleFill),
                snapshot.binding_hints.binding_for_action(Action::ToggleFill),
            )),
        });
    }

    x = draw_utility_row(layout, x, y, btn_size, icon_size, gap, is_simple);

    let icons_w = ToolbarLayoutSpec::TOP_TOGGLE_WIDTH;
    let icons_hover = hover
        .map(|(hx, hy)| point_in_rect(hx, hy, x, y, icons_w, btn_size))
        .unwrap_or(false);
    ctx.set_font_size(ICON_TOGGLE_FONT_SIZE);
    draw_checkbox(ctx, x, y, icons_w, btn_size, true, icons_hover, "Icons");
    ctx.set_font_size(TOP_LABEL_FONT_SIZE);
    layout.hits.push(HitRegion {
        rect: (x, y, icons_w, btn_size),
        event: ToolbarEvent::ToggleIconMode(false),
        kind: HitKind::Click,
        tooltip: None,
    });

    if is_simple && snapshot.shape_picker_open {
        draw_shape_picker_row(layout, handle_w, y, btn_size, icon_size);
    }
}
