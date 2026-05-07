use super::{HitKind, HitRegion, SideLayoutContext, ToolbarLayoutSpec, format_binding_label};
use crate::backend::wayland::toolbar::rows::{centered_grid_layout, grid_layout, row_item_width};
use crate::ui::toolbar::model::{
    ToolbarActionsModel, ToolbarCommandGroup, ToolbarCommandGroupKind,
};

pub(super) fn push_actions_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let Some(model) = ToolbarActionsModel::from_snapshot(ctx.snapshot) else {
        return y;
    };

    let actions_card_h = ctx.spec.side_actions_height(ctx.snapshot);
    let mut action_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let mut has_group = false;
    for group in model.groups() {
        if has_group {
            action_y += ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
        }

        let (next_y, has_rows) = if ctx.use_icons {
            push_icon_group_hits(ctx, action_y, hits, group)
        } else {
            push_text_group_hits(ctx, action_y, hits, group)
        };

        if has_rows {
            action_y = next_y;
            has_group = true;
        }
    }

    y + actions_card_h + ctx.section_gap
}

fn push_icon_group_hits(
    ctx: &SideLayoutContext<'_>,
    action_y: f64,
    hits: &mut Vec<HitRegion>,
    group: &ToolbarCommandGroup,
) -> (f64, bool) {
    let icon_btn = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON;
    let icon_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let columns = match group.kind {
        ToolbarCommandGroupKind::BasicActions => group.buttons.len(),
        ToolbarCommandGroupKind::ViewActions | ToolbarCommandGroupKind::AdvancedActions => 5,
        ToolbarCommandGroupKind::Pages | ToolbarCommandGroupKind::Boards => group.buttons.len(),
    };
    let layout = centered_grid_layout(
        ctx.x,
        ctx.content_width,
        action_y,
        icon_btn,
        icon_gap,
        columns,
        group.buttons.len(),
    );
    for (item, button) in layout.items.iter().zip(group.buttons.iter()) {
        push_button_hit(ctx, hits, item.x, item.y, item.w, item.h, button);
    }

    if layout.rows > 0 {
        (action_y + layout.height, true)
    } else {
        (action_y, false)
    }
}

fn push_text_group_hits(
    ctx: &SideLayoutContext<'_>,
    action_y: f64,
    hits: &mut Vec<HitRegion>,
    group: &ToolbarCommandGroup,
) -> (f64, bool) {
    let action_h = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT;
    let action_gap = ToolbarLayoutSpec::SIDE_ACTION_CONTENT_GAP_TEXT;
    let action_col_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let (action_w, columns, column_gap) = match group.kind {
        ToolbarCommandGroupKind::BasicActions => (ctx.content_width, 1, 0.0),
        ToolbarCommandGroupKind::ViewActions | ToolbarCommandGroupKind::AdvancedActions => (
            row_item_width(ctx.content_width, 2, action_col_gap),
            2,
            action_col_gap,
        ),
        ToolbarCommandGroupKind::Pages | ToolbarCommandGroupKind::Boards => {
            (ctx.content_width, group.buttons.len().max(1), 0.0)
        }
    };
    let layout = grid_layout(
        ctx.x,
        action_y,
        action_w,
        action_h,
        column_gap,
        action_gap,
        columns,
        group.buttons.len(),
    );
    for (item, button) in layout.items.iter().zip(group.buttons.iter()) {
        push_button_hit(ctx, hits, item.x, item.y, item.w, item.h, button);
    }

    if layout.rows > 0 {
        (action_y + layout.height, true)
    } else {
        (action_y, false)
    }
}

fn push_button_hit(
    ctx: &SideLayoutContext<'_>,
    hits: &mut Vec<HitRegion>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    button: &crate::ui::toolbar::model::ToolbarButtonModel,
) {
    if !button.enabled {
        return;
    }

    hits.push(HitRegion {
        rect: (x, y, w, h),
        event: button.event.clone(),
        kind: HitKind::Click,
        tooltip: Some(format_binding_label(
            button.tooltip_label(ctx.snapshot, "Action"),
            button.binding_hint(ctx.snapshot),
        )),
    });
}
