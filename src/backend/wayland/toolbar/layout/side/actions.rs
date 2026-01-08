use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec, format_binding_label,
};
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
            let icons_per_row = basic_actions.len();
            let total_icons_w =
                icons_per_row as f64 * icon_btn + (icons_per_row as f64 - 1.0) * icon_gap;
            let icons_start_x = ctx.x + (ctx.content_width - total_icons_w) / 2.0;
            for (idx, evt) in basic_actions.iter().enumerate() {
                let bx = icons_start_x + (icon_btn + icon_gap) * idx as f64;
                hits.push(HitRegion {
                    rect: (bx, action_y, icon_btn, icon_btn),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            action_y += icon_btn;
        } else {
            let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
            for (idx, evt) in basic_actions.iter().enumerate() {
                let by = action_y + (action_h + action_gap) * idx as f64;
                hits.push(HitRegion {
                    rect: (ctx.x, by, ctx.content_width, action_h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            action_y += action_h * basic_actions.len() as f64
                + action_gap * (basic_actions.len() as f64 - 1.0);
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
            let icons_per_row = 5usize;
            let total_icons = view_actions.len();
            let rows = if total_icons > 0 {
                total_icons.div_ceil(icons_per_row)
            } else {
                0
            };
            for (idx, evt) in view_actions.iter().enumerate() {
                let row = idx / icons_per_row;
                let col = idx % icons_per_row;
                let row_start = row * icons_per_row;
                let row_end = (row_start + icons_per_row).min(total_icons);
                let icons_in_row = row_end - row_start;
                let row_width =
                    icons_in_row as f64 * icon_btn + (icons_in_row as f64 - 1.0) * icon_gap;
                let icons_start_x = ctx.x + (ctx.content_width - row_width) / 2.0;
                let bx = icons_start_x + (icon_btn + icon_gap) * col as f64;
                let by = action_y + (icon_btn + icon_gap) * row as f64;
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn, icon_btn),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            if rows > 0 {
                action_y += icon_btn * rows as f64 + icon_gap * (rows as f64 - 1.0);
            }
        } else {
            let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
            let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
            let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let action_w = (ctx.content_width - action_col_gap) / 2.0;
            for (idx, evt) in view_actions.iter().enumerate() {
                let row = idx / 2;
                let col = idx % 2;
                let bx = ctx.x + (action_w + action_col_gap) * col as f64;
                let by = action_y + (action_h + action_gap) * row as f64;
                hits.push(HitRegion {
                    rect: (bx, by, action_w, action_h),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        event_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            let rows = view_actions.len().div_ceil(2);
            if rows > 0 {
                action_y += action_h * rows as f64 + action_gap * (rows as f64 - 1.0);
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
            let icons_per_row = 5usize;
            let total_icons = advanced_actions.len();
            for (idx, evt) in advanced_actions.iter().enumerate() {
                let row = idx / icons_per_row;
                let col = idx % icons_per_row;
                let row_start = row * icons_per_row;
                let row_end = (row_start + icons_per_row).min(total_icons);
                let icons_in_row = row_end - row_start;
                let row_width =
                    icons_in_row as f64 * icon_btn + (icons_in_row as f64 - 1.0) * icon_gap;
                let icons_start_x = ctx.x + (ctx.content_width - row_width) / 2.0;
                let bx = icons_start_x + (icon_btn + icon_gap) * col as f64;
                let by = action_y + (icon_btn + icon_gap) * row as f64;
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn, icon_btn),
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
            let action_w = (ctx.content_width - action_col_gap) / 2.0;
            for (idx, evt) in advanced_actions.iter().enumerate() {
                let row = idx / 2;
                let col = idx % 2;
                let bx = ctx.x + (action_w + action_col_gap) * col as f64;
                let by = action_y + (action_h + action_gap) * row as f64;
                hits.push(HitRegion {
                    rect: (bx, by, action_w, action_h),
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
