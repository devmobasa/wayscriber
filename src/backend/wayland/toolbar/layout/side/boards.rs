use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec, format_binding_label,
};
use crate::backend::wayland::toolbar::rows::{grid_layout, row_item_width};
use crate::config::action_label;
use crate::input::ToolbarDrawerTab;
use crate::ui::toolbar::bindings::action_for_event;

pub(super) fn push_boards_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if !ctx.snapshot.show_boards_section
        || !ctx.snapshot.drawer_open
        || ctx.snapshot.drawer_tab != ToolbarDrawerTab::View
    {
        return y;
    }

    let boards_card_h = ctx.spec.side_boards_height(ctx.snapshot);
    let boards_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let btn_h = if ctx.use_icons {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
    } else {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
    };
    let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let buttons = [
        ToolbarEvent::BoardPrev,
        ToolbarEvent::BoardNext,
        ToolbarEvent::BoardNew,
        ToolbarEvent::BoardDelete,
    ];
    let btn_w = row_item_width(ctx.content_width, buttons.len(), btn_gap);
    let layout = grid_layout(
        ctx.x,
        boards_y,
        btn_w,
        btn_h,
        btn_gap,
        0.0,
        buttons.len(),
        buttons.len(),
    );
    for (item, evt) in layout.items.iter().zip(buttons.iter()) {
        let tooltip_label = tooltip_label(evt);
        hits.push(HitRegion {
            rect: (item.x, item.y, item.w, item.h),
            event: evt.clone(),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                tooltip_label,
                ctx.snapshot.binding_hints.binding_for_event(evt),
            )),
        });
    }

    y + boards_card_h + ctx.section_gap
}

fn tooltip_label(event: &ToolbarEvent) -> &'static str {
    action_for_event(event).map(action_label).unwrap_or("Board")
}
