use super::{
    format_binding_label, HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec,
};

pub(super) fn push_actions_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let show_actions = ctx.snapshot.show_actions_section || ctx.snapshot.show_actions_advanced;
    if !show_actions {
        return y;
    }

    let actions_card_h = ctx.spec.side_actions_height(ctx.snapshot);
    let mut action_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let basic_actions = [
        ToolbarEvent::Undo,
        ToolbarEvent::Redo,
        ToolbarEvent::ClearCanvas,
    ];
    let advanced_actions = [
        ToolbarEvent::UndoAll,
        ToolbarEvent::RedoAll,
        ToolbarEvent::UndoAllDelayed,
        ToolbarEvent::RedoAllDelayed,
        ToolbarEvent::ToggleFreeze,
        ToolbarEvent::ZoomIn,
        ToolbarEvent::ZoomOut,
        ToolbarEvent::ResetZoom,
        ToolbarEvent::ToggleZoomLock,
    ];

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
                        action_label(evt, ctx.snapshot),
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
                        action_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
            action_y += action_h * basic_actions.len() as f64
                + action_gap * (basic_actions.len() as f64 - 1.0);
        }
    }

    if ctx.snapshot.show_actions_section && ctx.snapshot.show_actions_advanced {
        action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    }

    if ctx.snapshot.show_actions_advanced {
        if ctx.use_icons {
            let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
            let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
            let icons_per_row = 5usize;
            let total_icons_w =
                icons_per_row as f64 * icon_btn + (icons_per_row as f64 - 1.0) * icon_gap;
            let icons_start_x = ctx.x + (ctx.content_width - total_icons_w) / 2.0;
            for (idx, evt) in advanced_actions.iter().enumerate() {
                let row = idx / icons_per_row;
                let col = idx % icons_per_row;
                let bx = icons_start_x + (icon_btn + icon_gap) * col as f64;
                let by = action_y + (icon_btn + icon_gap) * row as f64;
                hits.push(HitRegion {
                    rect: (bx, by, icon_btn, icon_btn),
                    event: evt.clone(),
                    kind: HitKind::Click,
                    tooltip: Some(format_binding_label(
                        action_label(evt, ctx.snapshot),
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
                        action_label(evt, ctx.snapshot),
                        ctx.snapshot.binding_hints.binding_for_event(evt),
                    )),
                });
            }
        }
    }

    y + actions_card_h + ctx.section_gap
}

fn action_label(event: &ToolbarEvent, snapshot: &super::ToolbarSnapshot) -> &'static str {
    match event {
        ToolbarEvent::Undo => "Undo",
        ToolbarEvent::Redo => "Redo",
        ToolbarEvent::ClearCanvas => "Clear",
        ToolbarEvent::UndoAll => "Undo All",
        ToolbarEvent::RedoAll => "Redo All",
        ToolbarEvent::UndoAllDelayed => "Undo All Delay",
        ToolbarEvent::RedoAllDelayed => "Redo All Delay",
        ToolbarEvent::ToggleFreeze => {
            if snapshot.frozen_active {
                "Unfreeze"
            } else {
                "Freeze"
            }
        }
        ToolbarEvent::ZoomIn => "Zoom In",
        ToolbarEvent::ZoomOut => "Zoom Out",
        ToolbarEvent::ResetZoom => "Reset Zoom",
        ToolbarEvent::ToggleZoomLock => {
            if snapshot.zoom_locked {
                "Unlock Zoom"
            } else {
                "Lock Zoom"
            }
        }
        _ => "Action",
    }
}
