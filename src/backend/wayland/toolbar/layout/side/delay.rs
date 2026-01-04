use super::{
    HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec, delay_secs_from_t,
    delay_t_from_ms,
};

pub(super) fn push_delay_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    if ctx.snapshot.show_step_section
        && ctx.snapshot.show_delay_sliders
        && ctx.snapshot.drawer_open
        && ctx.snapshot.drawer_tab == crate::input::ToolbarDrawerTab::App
    {
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

    if ctx.snapshot.show_step_section
        && ctx.snapshot.drawer_open
        && ctx.snapshot.drawer_tab == crate::input::ToolbarDrawerTab::App
    {
        y + ctx.spec.side_step_height(ctx.snapshot) + ctx.section_gap
    } else {
        y
    }
}
