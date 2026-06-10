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
    let tabs = ToolbarDrawerTab::ALL;
    let tab_columns = 3usize;
    let tab_w =
        (ctx.content_width - tab_gap * (tab_columns - 1) as f64) / tab_columns as f64;

    for (idx, tab) in tabs.iter().enumerate() {
        let tab_col = idx % tab_columns;
        let tab_row = idx / tab_columns;
        let tab_x = ctx.x + (tab_w + tab_gap) * tab_col as f64;
        let tab_y = tab_y + (tab_h + tab_gap) * tab_row as f64;
        hits.push(HitRegion {
            rect: (tab_x, tab_y, tab_w, tab_h),
            event: ToolbarEvent::SetDrawerTab(*tab),
            kind: HitKind::Click,
            tooltip: Some(format!("Drawer: {}", tab.label())),
        });
    }

    y + tabs_h + ctx.section_gap
}
