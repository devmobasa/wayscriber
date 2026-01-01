use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};

pub(super) fn push_thickness_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    let slider_hit_h = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    hits.push(HitRegion {
        rect: (ctx.x, slider_row_y, ctx.content_width, slider_hit_h),
        event: ToolbarEvent::SetThickness(ctx.snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: 1.0,
            max: 50.0,
        },
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (
            ctx.x,
            slider_row_y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(-1.0),
        kind: HitKind::Click,
        tooltip: None,
    });
    hits.push(HitRegion {
        rect: (
            ctx.x + ctx.content_width - ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            slider_row_y,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::NudgeThickness(1.0),
        kind: HitKind::Click,
        tooltip: None,
    });

    y + ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT + ctx.section_gap
}

pub(super) fn push_text_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if !ctx.show_text_controls {
        return y;
    }

    let text_slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    hits.push(HitRegion {
        rect: (
            ctx.x,
            text_slider_row_y,
            ctx.content_width,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::SetFontSize(ctx.snapshot.font_size),
        kind: HitKind::DragSetFontSize,
        tooltip: None,
    });

    y + ToolbarLayoutSpec::SIDE_SLIDER_CARD_HEIGHT
        + ctx.section_gap
        + ToolbarLayoutSpec::SIDE_FONT_CARD_HEIGHT
        + ctx.section_gap
}
