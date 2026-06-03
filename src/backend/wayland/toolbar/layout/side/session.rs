use super::{HitKind, HitRegion, SideLayoutContext};
use crate::backend::wayland::toolbar::format_binding_label;
use crate::backend::wayland::toolbar::layout::ToolbarLayoutSpec;
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::ui::toolbar::model::ToolbarSessionModel;

pub(super) fn push_session_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let Some(model) = ToolbarSessionModel::from_snapshot(ctx.snapshot) else {
        return y;
    };

    let card_h = ctx.spec.side_session_height(ctx.snapshot);
    let mut row_y = y
        + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y
        + ToolbarLayoutSpec::SIDE_SESSION_META_HEIGHT
        + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;

    let button_gap = ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    let button_h = ToolbarLayoutSpec::SIDE_SESSION_BUTTON_HEIGHT;
    let columns = model.button_columns();
    let button_w = row_item_width(ctx.content_width, columns, button_gap);
    let button_layout = grid_layout(
        ctx.x,
        row_y,
        button_w,
        button_h,
        button_gap,
        button_gap,
        columns,
        model.buttons.len(),
    );
    for (item, button) in button_layout.items.iter().zip(model.buttons.iter()) {
        if button.enabled {
            hits.push(HitRegion {
                rect: (item.x, item.y, item.w, item.h),
                event: button.event.clone(),
                kind: HitKind::Click,
                tooltip: Some(button.label.to_string()),
            });
        }
    }
    row_y += button_layout.height + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;

    for recent in &model.recents {
        let tooltip_path = recent.path.display().to_string();
        hits.push(HitRegion {
            rect: (
                ctx.x,
                row_y,
                ctx.content_width,
                ToolbarLayoutSpec::SIDE_SESSION_RECENT_HEIGHT,
            ),
            event: recent.event(),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                &format!("Open {}", recent.label),
                Some(tooltip_path.as_str()),
            )),
        });
        row_y +=
            ToolbarLayoutSpec::SIDE_SESSION_RECENT_HEIGHT + ToolbarLayoutSpec::SIDE_SESSION_ROW_GAP;
    }

    y + card_h + ctx.section_gap
}
