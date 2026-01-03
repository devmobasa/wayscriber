use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};
use crate::input::ToolbarDrawerTab;

pub(super) fn push_drawer_tabs_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if !ctx.snapshot.drawer_open {
        return y;
    }

    let tabs_h = ctx.spec.side_drawer_tabs_height(ctx.snapshot);
    let tab_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let tab_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let tab_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let tab_w = (ctx.content_width - tab_gap) / 2.0;
    let tabs = [ToolbarDrawerTab::View, ToolbarDrawerTab::App];

    for (idx, tab) in tabs.iter().enumerate() {
        let tab_x = ctx.x + (tab_w + tab_gap) * idx as f64;
        hits.push(HitRegion {
            rect: (tab_x, tab_y, tab_w, tab_h),
            event: ToolbarEvent::SetDrawerTab(*tab),
            kind: HitKind::Click,
            tooltip: Some(format!("Drawer: {}", tab.label())),
        });
    }

    y + tabs_h + ctx.section_gap
}
