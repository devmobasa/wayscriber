use super::{HitKind, HitRegion, SideLayoutContext, ToolbarLayoutSpec, format_binding_label};
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::ui::toolbar::model::toolbar_boards_model;

pub(super) fn push_boards_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let Some(model) = toolbar_boards_model(ctx.snapshot) else {
        return y;
    };

    let boards_card_h = ctx.spec.side_boards_height(ctx.snapshot);
    let boards_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let btn_h = if ctx.use_icons {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
    } else {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
    };
    let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let cols = model.buttons.len().min(5).max(1);
    let btn_w = row_item_width(ctx.content_width, cols, btn_gap);
    let layout = grid_layout(
        ctx.x,
        boards_y,
        btn_w,
        btn_h,
        btn_gap,
        0.0,
        model.buttons.len(),
        cols,
    );
    for (item, button) in layout.items.iter().zip(model.buttons.iter()) {
        if !button.enabled {
            continue;
        }

        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: button.event.clone(),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                button.tooltip_label(ctx.snapshot, "Board"),
                button.binding_hint(ctx.snapshot),
            )),
        });
    }

    y + boards_card_h + ctx.section_gap
}
