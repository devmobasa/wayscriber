use super::{SideLayoutContext, ToolbarLayoutSpec};
use crate::input::Tool;

// Arrow hit regions are built in render to avoid duplicate registrations.
pub(super) fn advance_arrow_section(ctx: &SideLayoutContext<'_>, y: f64) -> f64 {
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
    y + card_h + ctx.section_gap
}
