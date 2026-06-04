use super::{HitKind, HitRegion, SideLayoutContext, ToolbarLayoutSpec};
use crate::ui::toolbar::{ToolbarEvent, ToolbarSideSection};

pub(super) fn push_collapsible_header_hit(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    section: ToolbarSideSection,
    hits: &mut Vec<HitRegion>,
) {
    let collapsed = ctx.snapshot.side_section_collapsed(section);
    hits.push(HitRegion {
        rect: (
            ctx.spec.side_card_x(),
            y,
            ctx.spec.side_card_width(ctx.width),
            ToolbarLayoutSpec::SIDE_COLLAPSE_HEADER_HIT_HEIGHT,
        ),
        event: ToolbarEvent::ToggleSideSectionCollapsed(section, !collapsed),
        kind: HitKind::Click,
        tooltip: Some(format!(
            "{} {}",
            if collapsed { "Expand" } else { "Collapse" },
            section.label()
        )),
    });
}
