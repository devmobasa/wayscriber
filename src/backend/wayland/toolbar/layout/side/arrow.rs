use super::{HitKind, HitRegion, SideLayoutContext, ToolbarEvent, ToolbarLayoutSpec};
use crate::input::Tool;

pub(super) fn push_arrow_hits(
    ctx: &SideLayoutContext<'_>,
    y: f64,
    hits: &mut Vec<HitRegion>,
) -> f64 {
    let show_arrow_controls =
        ctx.snapshot.active_tool == Tool::Arrow || ctx.snapshot.arrow_label_enabled;
    if !show_arrow_controls {
        return y;
    }

    let card_h = if ctx.snapshot.arrow_label_enabled {
        ToolbarLayoutSpec::SIDE_TOGGLE_CARD_HEIGHT_WITH_RESET
    } else {
        ToolbarLayoutSpec::SIDE_TOGGLE_CARD_HEIGHT
    };
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
