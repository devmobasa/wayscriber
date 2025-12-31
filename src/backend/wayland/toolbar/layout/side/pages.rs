use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec, format_binding_label,
};

pub(super) fn push_pages_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if !ctx.snapshot.show_actions_advanced {
        return y;
    }

    let pages_card_h = ctx.spec.side_pages_height(ctx.snapshot);
    let pages_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let btn_h = if ctx.use_icons {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_ICON
    } else {
        ToolbarLayoutSpec::SIDE_ACTION_BUTTON_HEIGHT_TEXT
    };
    let btn_gap = ToolbarLayoutSpec::SIDE_ACTION_BUTTON_GAP;
    let btn_w = (ctx.content_width - btn_gap * 4.0) / 5.0;
    let buttons = [
        (ToolbarEvent::PagePrev, "Prev"),
        (ToolbarEvent::PageNext, "Next"),
        (ToolbarEvent::PageNew, "New"),
        (ToolbarEvent::PageDuplicate, "Dup"),
        (ToolbarEvent::PageDelete, "Del"),
    ];
    for (idx, (evt, label)) in buttons.iter().enumerate() {
        let bx = ctx.x + (btn_w + btn_gap) * idx as f64;
        hits.push(HitRegion {
            rect: (bx, pages_y, btn_w, btn_h),
            event: evt.clone(),
            kind: HitKind::Click,
            tooltip: Some(format_binding_label(
                label,
                ctx.snapshot.binding_hints.binding_for_event(evt),
            )),
        });
    }

    y + pages_card_h + ctx.section_gap
}
