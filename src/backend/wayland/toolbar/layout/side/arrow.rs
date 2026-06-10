use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};
use crate::ui::toolbar::{ToolContext, ToolbarSideSection};

pub(super) fn push_arrow_section_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx
        .snapshot
        .side_section_hidden(ToolbarSideSection::ArrowLabels)
        || !ToolContext::from_snapshot(ctx.snapshot).show_arrow_labels
    {
        return y;
    }

    let card_h = ctx.spec.side_arrow_labels_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(
        ctx,
        y,
        ToolbarSideSection::ArrowLabels,
        hits,
    );
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::ArrowLabels)
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
        event: ToolbarEvent::ToggleArrowLabels(!ctx.snapshot.arrow_label_enabled),
        kind: HitKind::Click,
        tooltip: Some("Auto-number arrows 1, 2, 3.".to_string()),
    });
    if ctx.snapshot.arrow_label_enabled {
        let reset_y =
            toggle_y + ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT + ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
        hits.push(HitRegion {
            rect: (
                ctx.x,
                reset_y,
                ctx.content_width,
                ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT,
            ),
            event: ToolbarEvent::ResetArrowLabelCounter,
            kind: HitKind::Click,
            tooltip: Some("Reset numbering to 1.".to_string()),
        });
    }

    y + card_h + ctx.section_gap
}

pub(super) fn push_step_marker_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx
        .snapshot
        .side_section_hidden(ToolbarSideSection::StepMarkers)
        || !ToolContext::from_snapshot(ctx.snapshot).show_step_counter
    {
        return y;
    }

    let card_h = ctx.spec.side_step_markers_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(
        ctx,
        y,
        ToolbarSideSection::StepMarkers,
        hits,
    );
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::StepMarkers)
    {
        return y + card_h + ctx.section_gap;
    }

    let reset_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    hits.push(HitRegion {
        rect: (
            ctx.x,
            reset_y,
            ctx.content_width,
            ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT,
        ),
        event: ToolbarEvent::ResetStepMarkerCounter,
        kind: HitKind::Click,
        tooltip: Some("Reset numbering to 1.".to_string()),
    });

    y + card_h + ctx.section_gap
}
