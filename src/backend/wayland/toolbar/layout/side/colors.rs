use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};
use crate::ui::toolbar::ToolbarSideSection;

pub(super) fn push_color_picker_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let card_h = ctx.spec.side_colors_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(ctx, y, ToolbarSideSection::Colors, hits);
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::Colors)
    {
        return y + card_h + ctx.section_gap;
    }

    let picker_y = y + ToolbarLayoutSpec::SIDE_COLOR_PICKER_OFFSET_Y;
    let picker_h = ctx.spec.side_color_picker_height(ctx.snapshot);
    hits.push(HitRegion {
        rect: (ctx.x, picker_y, ctx.content_width, picker_h),
        event: ToolbarEvent::SetColor(ctx.snapshot.color),
        kind: HitKind::PickColor {
            x: ctx.x,
            y: picker_y,
            w: ctx.content_width,
            h: picker_h,
        },
        tooltip: None,
    });

    y + card_h + ctx.section_gap
}
