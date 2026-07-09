use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::hit::HitRegion;
use crate::backend::wayland::toolbar::rows::{centered_grid_layout, grid_layout};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSnapshot};
use crate::ui_text::UiTextStyle;

use super::super::super::widgets::constants::{COLOR_TEXT_DISABLED, set_color};
use super::super::super::widgets::{
    draw_button, draw_destructive_button, draw_disabled_button, draw_label_center,
    draw_label_center_color, point_in_rect, set_icon_color,
};

pub(super) type ActionIconFn = fn(&cairo::Context, f64, f64, f64);

#[derive(Clone)]
pub(super) struct ActionButton {
    pub(super) event: ToolbarEvent,
    pub(super) icon_fn: ActionIconFn,
    pub(super) enabled: bool,
}

pub(super) struct IconActionLayout {
    pub(super) x: f64,
    pub(super) content_width: f64,
    pub(super) start_y: f64,
    pub(super) button_size: f64,
    pub(super) icon_size: f64,
    pub(super) gap: f64,
    pub(super) columns: usize,
    pub(super) add_gap: bool,
}

#[derive(Copy, Clone)]
struct IconActionButtonGeometry {
    x: f64,
    y: f64,
    button_size: f64,
    icon_size: f64,
}

pub(super) struct TextActionLayout {
    pub(super) x: f64,
    pub(super) start_y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) column_gap: f64,
    pub(super) group_gap: f64,
    pub(super) row_gap: f64,
    pub(super) columns: usize,
    pub(super) add_gap: bool,
    pub(super) label_style: UiTextStyle<'static>,
    pub(super) enabled_style: bool,
}

#[derive(Copy, Clone)]
struct TextActionButtonGeometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

struct ActionButtonRenderContext<'a> {
    ctx: &'a cairo::Context,
    hits: &'a mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    snapshot: &'a ToolbarSnapshot,
}

pub(super) fn render_icon_action_group(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    snapshot: &ToolbarSnapshot,
    layout: IconActionLayout,
    actions: &[ActionButton],
) -> (f64, bool) {
    let mut render_ctx = ActionButtonRenderContext {
        ctx,
        hits,
        hover,
        snapshot,
    };
    let mut action_y = layout.start_y;
    if layout.add_gap {
        action_y += layout.gap;
    }

    let grid = centered_grid_layout(
        layout.x,
        layout.content_width,
        action_y,
        layout.button_size,
        layout.gap,
        layout.columns,
        actions.len(),
    );
    for (item, action) in grid.items.iter().zip(actions.iter()) {
        render_icon_action_button(
            &mut render_ctx,
            IconActionButtonGeometry {
                x: item.x,
                y: item.y,
                button_size: layout.button_size,
                icon_size: layout.icon_size,
            },
            action,
        );
    }

    if grid.rows > 0 {
        (action_y + grid.height, true)
    } else {
        (action_y, false)
    }
}

pub(super) fn render_text_action_group(
    ctx: &cairo::Context,
    hits: &mut Vec<HitRegion>,
    hover: Option<(f64, f64)>,
    snapshot: &ToolbarSnapshot,
    layout: TextActionLayout,
    actions: &[ActionButton],
) -> (f64, bool) {
    let mut render_ctx = ActionButtonRenderContext {
        ctx,
        hits,
        hover,
        snapshot,
    };
    let mut action_y = layout.start_y;
    if layout.add_gap {
        action_y += layout.group_gap;
    }

    let grid = grid_layout(
        layout.x,
        action_y,
        layout.width,
        layout.height,
        layout.column_gap,
        layout.row_gap,
        layout.columns,
        actions.len(),
    );
    for (item, action) in grid.items.iter().zip(actions.iter()) {
        render_text_action_button(
            &mut render_ctx,
            layout.label_style,
            TextActionButtonGeometry {
                x: item.x,
                y: item.y,
                width: item.w,
                height: item.h,
            },
            action,
            layout.enabled_style,
        );
    }

    if grid.rows > 0 {
        (action_y + grid.height, true)
    } else {
        (action_y, false)
    }
}

pub(super) fn button_label(event: &ToolbarEvent, snapshot: &ToolbarSnapshot) -> &'static str {
    event.short_label(snapshot, "Action")
}

fn tooltip_label(event: &ToolbarEvent, snapshot: &ToolbarSnapshot) -> &'static str {
    event.tooltip_label(snapshot, "Action")
}

fn render_icon_action_button(
    render_ctx: &mut ActionButtonRenderContext<'_>,
    geometry: IconActionButtonGeometry,
    action: &ActionButton,
) {
    let is_hover = render_ctx
        .hover
        .map(|(hx, hy)| {
            point_in_rect(
                hx,
                hy,
                geometry.x,
                geometry.y,
                geometry.button_size,
                geometry.button_size,
            )
        })
        .unwrap_or(false);

    let is_destructive = action.event.is_destructive();
    if action.enabled {
        if is_destructive {
            draw_destructive_button(
                render_ctx.ctx,
                geometry.x,
                geometry.y,
                geometry.button_size,
                geometry.button_size,
                is_hover,
            );
        } else {
            draw_button(
                render_ctx.ctx,
                geometry.x,
                geometry.y,
                geometry.button_size,
                geometry.button_size,
                false,
                is_hover,
            );
        }
        set_icon_color(render_ctx.ctx, is_hover);
    } else {
        draw_disabled_button(
            render_ctx.ctx,
            geometry.x,
            geometry.y,
            geometry.button_size,
            geometry.button_size,
        );
        set_color(render_ctx.ctx, COLOR_TEXT_DISABLED);
    }

    let icon_x = geometry.x + (geometry.button_size - geometry.icon_size) / 2.0;
    let icon_y = geometry.y + (geometry.button_size - geometry.icon_size) / 2.0;
    (action.icon_fn)(render_ctx.ctx, icon_x, icon_y, geometry.icon_size);

    if action.enabled {
        render_ctx.hits.push(HitRegion {
            rect: (
                geometry.x,
                geometry.y,
                geometry.button_size,
                geometry.button_size,
            ),
            event: action.event.clone(),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tooltip_label(&action.event, render_ctx.snapshot),
                render_ctx
                    .snapshot
                    .binding_hints
                    .binding_for_event(&action.event),
            )),
        });
    }
}

fn render_text_action_button(
    render_ctx: &mut ActionButtonRenderContext<'_>,
    label_style: UiTextStyle<'static>,
    layout: TextActionButtonGeometry,
    action: &ActionButton,
    enabled_style: bool,
) {
    let label = button_label(&action.event, render_ctx.snapshot);
    let is_hover = render_ctx
        .hover
        .map(|(hx, hy)| point_in_rect(hx, hy, layout.x, layout.y, layout.width, layout.height))
        .unwrap_or(false);

    if !action.enabled {
        draw_disabled_button(
            render_ctx.ctx,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
        );
    } else if action.event.is_destructive() {
        draw_destructive_button(
            render_ctx.ctx,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            is_hover,
        );
    } else {
        draw_button(
            render_ctx.ctx,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            enabled_style,
            is_hover,
        );
    }

    if action.enabled {
        draw_label_center(
            render_ctx.ctx,
            label_style,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            label,
        );
    } else {
        draw_label_center_color(
            render_ctx.ctx,
            label_style,
            layout.x,
            layout.y,
            layout.width,
            layout.height,
            label,
            COLOR_TEXT_DISABLED,
        );
    }
    if action.enabled {
        render_ctx.hits.push(HitRegion {
            rect: (layout.x, layout.y, layout.width, layout.height),
            event: action.event.clone(),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tooltip_label(&action.event, render_ctx.snapshot),
                render_ctx
                    .snapshot
                    .binding_hints
                    .binding_for_event(&action.event),
            )),
        });
    }
}
