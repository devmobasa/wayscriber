use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};
use crate::ui::toolbar::model::ToolbarSliderSpec;
use crate::ui::toolbar::{ToolContext, ToolbarSideSection};

pub(super) fn push_thickness_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx
        .snapshot
        .side_section_hidden(ToolbarSideSection::Thickness)
    {
        return y;
    }

    let card_h = ctx.spec.side_thickness_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(ctx, y, ToolbarSideSection::Thickness, hits);
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::Thickness)
    {
        return y + card_h + ctx.section_gap;
    }

    let slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    let slider_hit_h = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    hits.push(HitRegion {
        rect: (ctx.x, slider_row_y, ctx.content_width, slider_hit_h),
        event: ToolbarEvent::SetThickness(ctx.snapshot.thickness),
        kind: HitKind::DragSetThickness {
            min: ToolbarSliderSpec::THICKNESS.min,
            max: ToolbarSliderSpec::THICKNESS.max,
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
        event: ToolbarEvent::NudgeThickness(-ToolbarSliderSpec::THICKNESS.step.unwrap_or(1.0)),
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
        event: ToolbarEvent::NudgeThickness(ToolbarSliderSpec::THICKNESS.step.unwrap_or(1.0)),
        kind: HitKind::Click,
        tooltip: None,
    });

    y + card_h + ctx.section_gap
}

pub(super) fn push_eraser_mode_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx
        .snapshot
        .side_section_hidden(ToolbarSideSection::EraserMode)
    {
        return y;
    }

    let card_h = ctx.spec.side_eraser_mode_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(
        ctx,
        y,
        ToolbarSideSection::EraserMode,
        hits,
    );
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::EraserMode)
    {
        return y + card_h + ctx.section_gap;
    }

    let toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    hits.push(HitRegion {
        rect: (
            ctx.x,
            toggle_y,
            ctx.content_width,
            ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT,
        ),
        event: ToolbarEvent::SetEraserMode(
            if ctx.snapshot.eraser_mode == crate::input::EraserMode::Stroke {
                crate::input::EraserMode::Brush
            } else {
                crate::input::EraserMode::Stroke
            },
        ),
        kind: HitKind::Click,
        tooltip: Some("Erase by stroke".to_string()),
    });

    y + card_h + ctx.section_gap
}

pub(super) fn push_polygon_sides_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx
        .snapshot
        .side_section_hidden(ToolbarSideSection::PolygonSides)
    {
        return y;
    }

    let card_h = ctx.spec.side_polygon_sides_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(
        ctx,
        y,
        ToolbarSideSection::PolygonSides,
        hits,
    );
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::PolygonSides)
    {
        return y + card_h + ctx.section_gap;
    }

    let row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    let btn = ToolbarLayoutSpec::SIDE_NUDGE_SIZE;
    hits.push(HitRegion {
        rect: (ctx.x, row_y, btn, btn),
        event: ToolbarEvent::NudgePolygonSides(-1),
        kind: HitKind::Click,
        tooltip: Some("Decrease polygon sides".to_string()),
    });
    hits.push(HitRegion {
        rect: (ctx.x + ctx.content_width - btn, row_y, btn, btn),
        event: ToolbarEvent::NudgePolygonSides(1),
        kind: HitKind::Click,
        tooltip: Some("Increase polygon sides".to_string()),
    });
    hits.push(HitRegion {
        rect: (ctx.x + btn, row_y, ctx.content_width - btn * 2.0, btn),
        event: ToolbarEvent::SetPolygonSides(ctx.snapshot.polygon_sides),
        kind: HitKind::Click,
        tooltip: None,
    });

    y + card_h + ctx.section_gap
}

pub(super) fn push_text_size_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx.snapshot.side_section_hidden(ToolbarSideSection::TextSize)
        || !ToolContext::from_snapshot(ctx.snapshot).show_font_controls
    {
        return y;
    }

    let card_h = ctx.spec.side_text_size_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(ctx, y, ToolbarSideSection::TextSize, hits);
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::TextSize)
    {
        return y + card_h + ctx.section_gap;
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

    y + card_h + ctx.section_gap
}

pub(super) fn push_marker_opacity_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx
        .snapshot
        .side_section_hidden(ToolbarSideSection::MarkerOpacity)
        || !ToolContext::from_snapshot(ctx.snapshot).show_marker_opacity
    {
        return y;
    }

    let card_h = ctx.spec.side_marker_opacity_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(
        ctx,
        y,
        ToolbarSideSection::MarkerOpacity,
        hits,
    );
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::MarkerOpacity)
    {
        return y + card_h + ctx.section_gap;
    }

    let slider_row_y = y + ToolbarLayoutSpec::SIDE_SLIDER_ROW_OFFSET;
    hits.push(HitRegion {
        rect: (
            ctx.x,
            slider_row_y,
            ctx.content_width,
            ToolbarLayoutSpec::SIDE_NUDGE_SIZE,
        ),
        event: ToolbarEvent::SetMarkerOpacity(ctx.snapshot.marker_opacity),
        kind: HitKind::DragSetMarkerOpacity {
            min: ToolbarSliderSpec::MARKER_OPACITY.min,
            max: ToolbarSliderSpec::MARKER_OPACITY.max,
        },
        tooltip: None,
    });

    y + card_h + ctx.section_gap
}

pub(super) fn push_font_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx.snapshot.side_section_hidden(ToolbarSideSection::Font)
        || !ToolContext::from_snapshot(ctx.snapshot).show_font_controls
    {
        return y;
    }

    let card_h = ctx.spec.side_font_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(ctx, y, ToolbarSideSection::Font, hits);
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::Font)
    {
        return y + card_h + ctx.section_gap;
    }

    let font_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    let font_gap = ToolbarLayoutSpec::SIDE_FONT_BUTTON_GAP;
    let font_w = (ctx.content_width - font_gap) / 2.0;
    for idx in 0..2 {
        hits.push(HitRegion {
            rect: (
                ctx.x + idx as f64 * (font_w + font_gap),
                font_y,
                font_w,
                ToolbarLayoutSpec::SIDE_FONT_BUTTON_HEIGHT,
            ),
            event: ToolbarEvent::SetFont(if idx == 0 {
                crate::draw::FontDescriptor::new(
                    "Sans".to_string(),
                    "bold".to_string(),
                    "normal".to_string(),
                )
            } else {
                crate::draw::FontDescriptor::new(
                    "Monospace".to_string(),
                    "normal".to_string(),
                    "normal".to_string(),
                )
            }),
            kind: HitKind::Click,
            tooltip: None,
        });
    }

    y + card_h + ctx.section_gap
}
