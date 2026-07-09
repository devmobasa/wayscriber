use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec, delay_secs_from_t,
    delay_t_from_ms,
};
use crate::ui::toolbar::ToolbarSideSection;

pub(super) fn push_delay_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx
        .snapshot
        .side_section_hidden(ToolbarSideSection::StepUndo)
        || !ctx.snapshot.show_step_section
        || !ctx.snapshot.drawer_open
        || ctx.snapshot.drawer_tab != crate::input::ToolbarDrawerTab::App
    {
        return y;
    }

    let card_h = ctx.spec.side_step_height(ctx.snapshot);
    super::section_header::push_collapsible_header_hit(ctx, y, ToolbarSideSection::StepUndo, hits);
    if ctx
        .snapshot
        .side_section_collapsed(ToolbarSideSection::StepUndo)
    {
        return y + card_h + ctx.section_gap;
    }

    let toggle_h = ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT;
    let toggle_gap = ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let custom_toggle_y = y + ToolbarLayoutSpec::SIDE_SECTION_TOGGLE_OFFSET_Y;
    hits.push(HitRegion {
        rect: (ctx.x, custom_toggle_y, ctx.content_width, toggle_h),
        event: ToolbarEvent::ToggleCustomSection(!ctx.snapshot.custom_section_enabled),
        kind: HitKind::Click,
        tooltip: Some("Step buttons: undo/redo several strokes at once.".to_string()),
    });
    let delay_toggle_y = custom_toggle_y + toggle_h + toggle_gap;
    hits.push(HitRegion {
        rect: (ctx.x, delay_toggle_y, ctx.content_width, toggle_h),
        event: ToolbarEvent::ToggleDelaySliders(!ctx.snapshot.show_delay_sliders),
        kind: HitKind::Click,
        tooltip: Some("Delay sliders: undo/redo delays.".to_string()),
    });

    if ctx.snapshot.show_delay_sliders {
        push_delay_slider_hits(ctx, y, hits);
    }

    y + card_h + ctx.section_gap
}

fn push_delay_slider_hits(ctx: &SideLayoutContext<'_>, y: f64, hits: &mut Vec<HitRegion>) {
    let undo_t = delay_t_from_ms(ctx.snapshot.undo_all_delay_ms);
    let redo_t = delay_t_from_ms(ctx.snapshot.redo_all_delay_ms);
    let toggles_h =
        ToolbarLayoutSpec::SIDE_TOGGLE_HEIGHT * 2.0 + ToolbarLayoutSpec::SIDE_TOGGLE_GAP;
    let custom_h = if ctx.snapshot.custom_section_enabled {
        ToolbarLayoutSpec::SIDE_CUSTOM_SECTION_HEIGHT
    } else {
        0.0
    };
    let slider_start_y = y
        + ToolbarLayoutSpec::SIDE_STEP_HEADER_HEIGHT
        + toggles_h
        + custom_h
        + ToolbarLayoutSpec::SIDE_STEP_SLIDER_TOP_PADDING;
    let slider_hit_h = ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HEIGHT
        + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING * 2.0;
    let undo_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_UNDO_OFFSET_Y;
    hits.push(HitRegion {
        rect: (
            ctx.x,
            undo_y - ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING,
            ctx.content_width,
            slider_hit_h,
        ),
        event: ToolbarEvent::SetUndoDelay(delay_secs_from_t(undo_t)),
        kind: HitKind::DragUndoDelay,
        tooltip: None,
    });
    let redo_y = slider_start_y + ToolbarLayoutSpec::SIDE_DELAY_SLIDER_REDO_OFFSET_Y;
    hits.push(HitRegion {
        rect: (
            ctx.x,
            redo_y - ToolbarLayoutSpec::SIDE_DELAY_SLIDER_HIT_PADDING,
            ctx.content_width,
            slider_hit_h,
        ),
        event: ToolbarEvent::SetRedoDelay(delay_secs_from_t(redo_t)),
        kind: HitKind::DragRedoDelay,
        tooltip: None,
    });
}
