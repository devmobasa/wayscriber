use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec, format_binding_label,
};
use crate::backend::wayland::toolbar::rows::{centered_grid_layout, grid_layout, row_item_width};
use crate::config::{Action, action_label, action_short_label};
use crate::input::ToolbarDrawerTab;
use crate::ui::toolbar::bindings::action_for_event;

pub(super) fn push_actions_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let show_drawer_view =
        ctx.snapshot.drawer_open && ctx.snapshot.drawer_tab == ToolbarDrawerTab::View;
    let show_advanced = ctx.snapshot.show_actions_advanced && show_drawer_view;
    let show_view_actions = show_drawer_view
        && ctx.snapshot.show_zoom_actions
        && (ctx.snapshot.show_actions_section || ctx.snapshot.show_actions_advanced);
    let show_actions = ctx.snapshot.show_actions_section || show_advanced;
    if !show_actions {
        return y;
    }

    let mut actions_snapshot = ctx.snapshot.clone();
    actions_snapshot.show_actions_advanced = show_advanced;
    let actions_card_h = ctx.spec.side_actions_height(&actions_snapshot);
    let mut action_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let basic_actions = [
        ToolbarEvent::Undo,
        ToolbarEvent::Redo,
        ToolbarEvent::ClearCanvas,
    ];
    let view_actions = [
        ToolbarEvent::ZoomIn,
        ToolbarEvent::ZoomOut,
        ToolbarEvent::ResetZoom,
        ToolbarEvent::ToggleZoomLock,
    ];
    let mut advanced_actions = Vec::new();
    advanced_actions.push(ToolbarEvent::UndoAll);
    advanced_actions.push(ToolbarEvent::RedoAll);
    if ctx.snapshot.delay_actions_enabled {
        advanced_actions.push(ToolbarEvent::UndoAllDelayed);
        advanced_actions.push(ToolbarEvent::RedoAllDelayed);
    }
    advanced_actions.push(ToolbarEvent::ToggleFreeze);

    if ctx.snapshot.show_actions_section {
        if ctx.use_icons {
            let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let layout = centered_grid_layout(
                ctx.x,
                ctx.content_width,
                action_y,
                icon_btn,
                icon_gap,
                basic_actions.len(),
                basic_actions.len(),
            );
            for (item, evt) in layout.items.iter().zip(basic_actions.iter()) {
                hits.push(HitRegion {
                    rect: (item.x, item.y, item.w, item.h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            action_y += layout.height;
        } else {
            let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
            let layout = grid_layout(
                ctx.x,
                action_y,
                ctx.content_width,
                action_h,
                0.0,
                action_gap,
                1,
                basic_actions.len(),
            );
            for (item, evt) in layout.items.iter().zip(basic_actions.iter()) {
                hits.push(HitRegion {
                    rect: (item.x, item.y, item.w, item.h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            action_y += layout.height;
        }
    }

    let mut has_group = ctx.snapshot.show_actions_section;

    if show_view_actions {
        if has_group {
            action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }
        if ctx.use_icons {
            let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let layout = centered_grid_layout(
                ctx.x,
                ctx.content_width,
                action_y,
                icon_btn,
                icon_gap,
                5,
                view_actions.len(),
            );
            for (item, evt) in layout.items.iter().zip(view_actions.iter()) {
                hits.push(HitRegion {
                    rect: (item.x, item.y, item.w, item.h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            if layout.rows > 0 {
                action_y += layout.height;
            }
        } else {
            let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
            let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let action_w = row_item_width(ctx.content_width, 2, action_col_gap);
            let layout = grid_layout(
                ctx.x,
                action_y,
                action_w,
                action_h,
                action_col_gap,
                action_gap,
                2,
                view_actions.len(),
            );
            for (item, evt) in layout.items.iter().zip(view_actions.iter()) {
                hits.push(HitRegion {
                    rect: (item.x, item.y, item.w, item.h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            if layout.rows > 0 {
                action_y += layout.height;
            }
        }
        has_group = true;
    }

    if show_advanced {
        if has_group {
            action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }
        if ctx.use_icons {
            let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let layout = centered_grid_layout(
                ctx.x,
                ctx.content_width,
                action_y,
                icon_btn,
                icon_gap,
                5,
                advanced_actions.len(),
            );
            for (item, evt) in layout.items.iter().zip(advanced_actions.iter()) {
                hits.push(HitRegion {
                    rect: (item.x, item.y, item.w, item.h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
        } else {
            let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
            let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let action_w = row_item_width(ctx.content_width, 2, action_col_gap);
            let layout = grid_layout(
                ctx.x,
                action_y,
                action_w,
                action_h,
                action_col_gap,
                action_gap,
                2,
                advanced_actions.len(),
            );
            for (item, evt) in layout.items.iter().zip(advanced_actions.iter()) {
                hits.push(HitRegion {
                    rect: (item.x, item.y, item.w, item.h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
        }
    }

    y + actions_card_h + ctx.section_gap
}

fn event_label(event: &ToolbarEvent, snapshot: &super::ToolbarSnapshot) -> &'static str {
    match event {
        ToolbarEvent::ToggleFreeze => {
            if snapshot.frozen_active {
                "Unfreeze"
            } else {
                action_short_label(Action::ToggleFrozenMode)
            }
        }
        ToolbarEvent::ToggleZoomLock => {
            if snapshot.zoom_locked {
                "Unlock Zoom"
            } else {
                action_label(Action::ToggleZoomLock)
            }
        }
        _ => action_for_event(event)
            .map(action_label)
            .unwrap_or("Action"),
    }
}
